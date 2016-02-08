use postgres::{Connection, GenericConnection};
use chrono::{DateTime, UTC};
use chrono::duration::Duration;

use model::{first, first_opt, User};
use error::{FictResult, FictError, fict_err};

/// An ordered sequence of Snippets that combine to form a (hopefully) hilarious piece of fiction.
pub struct Story {
    pub id: i64,
    pub title: Option<String>,
    pub published: bool,
    pub world_readable: bool,
    pub lock_duration_s: i64,
    pub contribution_count: i32,
    pub creation_time: DateTime<UTC>,
    pub update_time: DateTime<UTC>,
    pub publish_time: Option<DateTime<UTC>>,
    pub lock_user_id: Option<i64>,
    pub lock_expiration: Option<DateTime<UTC>>
}

impl Story {

    /// Initialize database tables and indices used to story `Story` objects.
    ///
    /// Depends on `User::initialize`.
    pub fn initialize(conn: &GenericConnection) -> FictResult<()> {
        try!(conn.execute("
            CREATE TABLE IF NOT EXISTS stories (
                id BIGSERIAL PRIMARY KEY,
                title VARCHAR,
                published BOOLEAN NOT NULL DEFAULT false,
                world_readable BOOLEAN NOT NULL DEFAULT false,
                lock_duration_s BIGINT NOT NULL DEFAULT 21600,
                contribution_count INT NOT NULL DEFAULT 0,
                creation_time TIMESTAMP WITH TIME ZONE NOT NULL
                    DEFAULT (now() AT TIME ZONE 'utc'),
                update_time TIMESTAMP WITH TIME ZONE NOT NULL
                    DEFAULT (now() AT TIME ZONE 'utc'),
                publish_time TIMESTAMP WITH TIME ZONE,
                lock_user_id BIGINT REFERENCES users (id)
                    ON DELETE SET NULL
                    ON UPDATE CASCADE,
                lock_expiration TIMESTAMP WITH TIME ZONE
            )
        ", &[]));

        try!(conn.execute("
            CREATE INDEX IF NOT EXISTS stories_lock_index ON stories (lock_user_id)
        ", &[]));

        Ok(())
    }

    /// Create and persist a new `Story`. The provided `User` will be granted Owner-level access
    /// to the story.
    pub fn begin(conn: &GenericConnection, owner: &User) -> FictResult<Story> {
        let insertion = try!(conn.prepare("
            INSERT INTO stories DEFAULT VALUES
            RETURNING
                id, title, published, world_readable, lock_duration_s, contribution_count,
                creation_time, update_time,
                publish_time, lock_user_id, lock_expiration
        "));

        let rows = try!(insertion.query(&[]));
        let row = try!(first(&rows));

        let story = Story{
            id: row.get(0),
            title: row.get(1),
            published: row.get(2),
            world_readable: row.get(3),
            lock_duration_s: row.get(4),
            contribution_count: row.get(5),
            creation_time: row.get(6),
            update_time: row.get(7),
            publish_time: row.get(8),
            lock_user_id: row.get(9),
            lock_expiration: row.get(10)
        };

        // Automatically grant Owner access to the creating user.
        try!(StoryAccess::grant(conn, &story, owner, &AccessLevel::Owner));

        Ok(story)
    }

    /// Search for an existing `Story` by ID and ensure that a User holds an unexpired lock on that
    /// story.
    ///
    /// If the story does not exist, or if the current user does not have sufficient access to write
    /// to this story, return `Err(FictError::NotFound)`.
    ///
    /// If the story is currently locked by someone else, return `Err(FictError::LockFailure)` with
    /// the lock details.
    ///
    /// If `acquire` is `false` and the story is not locked, return `Err(FictError::Unlocked)`.
    ///
    /// If the applicant has locked the story for contribution before and no other User has
    /// contributed an intervening Snippet, return `Err(FictError::Cooldown)`.
    ///
    /// Otherwise, atomically acquire the Story lock on behalf of the applicant User.
    pub fn locked_for_write(conn: &Connection, id: i64, applicant: &User, acquire: bool) -> FictResult<Story> {
        let now = UTC::now();
        let transaction = try!(conn.transaction());

        // Locate and lock the story row.
        let selection = try!(transaction.prepare("
            SELECT
                id, title, published, world_readable, lock_duration_s, contribution_count,
                creation_time, update_time, publish_time,
                lock_user_id, lock_expiration
            FROM stories
            WHERE id = $1
            FOR UPDATE
        "));

        let selection_rows = try!(selection.query(&[&id]));
        let story_opt = try!(first_opt(&selection_rows)).map(|row| Story{
            id: row.get(0),
            title: row.get(1),
            published: row.get(2),
            world_readable: row.get(3),
            lock_duration_s: row.get(4),
            contribution_count: row.get(5),
            creation_time: row.get(6),
            update_time: row.get(7),
            publish_time: row.get(8),
            lock_user_id: row.get(9),
            lock_expiration: row.get(10)
        });

        // Story ID does not match a known story.
        if story_opt.is_none() {
            return Err(FictError::NotFound);
        }
        let mut story = story_opt.unwrap();

        // Applicant does not have sufficient permission to lock this story.
        let access = try!(story.access_for(conn, applicant));
        if ! access.grants_write() {
            return Err(FictError::NotFound);
        }

        // Applicant is not a persisted user. Caller error.
        let applicant_id = try!(applicant.id.ok_or(
            fict_err(format!("User {} must be persisted to lock a story.", applicant.name))
        ));

        // Story is currently locked by a different user with an unexpired lock.
        let locked_by_other = story.lock_user_id.map(|owner_id| {
            owner_id != applicant_id
        }).unwrap_or(false);

        let expiration_is_valid = story.lock_expiration.map(|exp| {
            exp >= now
        }).unwrap_or(false);

        if locked_by_other && ! expiration_is_valid {
            let owner = try!(User::with_id(&transaction, story.lock_user_id.unwrap()));

            return Err(FictError::AlreadyLocked {
                username: owner.name,
                expiration: story.lock_expiration.unwrap()
            });
        }

        // Story is unlocked and no lock was requested.
        if ! acquire {
            let locked_by_applicant = story.lock_user_id.map(|owner_id| {
                owner_id == applicant_id
            }).unwrap_or(false);

            if ! locked_by_applicant && ! expiration_is_valid {
                return Err(FictError::Unlocked);
            };
        }

        // Ensure that at least one Snippet has been contributed since the last time the applicant
        // locked the Story for contribution.
        //
        // Permit the lock to continue if:
        // 1. applicant has already seen this Snippet
        //    (attempt == contribution count)
        // OR
        // 2. at least one other Snippet has been contributed since the last attempt
        //    (attempt + 2 <= contribution count)
        // OR
        // 3. applicant has *never* locked the story (None)
        let at_least_one_between =
            try!(ContributionAttempt::most_recent_attempt(conn, &story, &applicant))
            .map(|attempt| attempt == story.contribution_count || attempt + 2 <= story.contribution_count)
            .unwrap_or(true); // No prior contributon attempts.

        if ! at_least_one_between {
            return Err(FictError::Cooldown);
        }

        // Acquire the story lock and compute a new expiration.
        let update = try!(transaction.prepare("
            UPDATE stories
            SET
                lock_user_id = $1,
                lock_expiration = $2
            WHERE id = $3
        "));

        let lock_expiration = now + Duration::seconds(story.lock_duration_s);

        try!(update.query(&[&applicant_id, &lock_expiration, &story.id]));
        story.lock_user_id = Some(applicant_id);
        story.lock_expiration = Some(lock_expiration);

        try!(transaction.commit());

        // Return the locked story.
        Ok(story)
    }

    /// Search for an existing `Story` by ID.
    pub fn with_id(conn: &GenericConnection, id: i64) -> FictResult<Option<Story>> {
        let selection = try!(conn.prepare("
            SELECT
                id, title, published, world_readable, lock_duration_s, contribution_count,
                creation_time, update_time, publish_time,
                lock_user_id, lock_expiration
            FROM stories
            WHERE id = $1
        "));

        let rows = try!(selection.query(&[&id]));
        let row_opt = try!(first_opt(&rows));

        Ok(row_opt
            .map(|row| Story{
                id: row.get(0),
                title: row.get(1),
                published: row.get(2),
                world_readable: row.get(3),
                lock_duration_s: row.get(4),
                contribution_count: row.get(5),
                creation_time: row.get(6),
                update_time: row.get(7),
                publish_time: row.get(8),
                lock_user_id: row.get(9),
                lock_expiration: row.get(10)
            }))
    }

    /// Determine the level of access granted to a given `User`.
    pub fn access_for(&self, conn: &GenericConnection, user: &User) -> FictResult<AccessLevel> {
        let access = try!(StoryAccess::access_for(conn, user, &self));

        Ok(if self.published && self.world_readable {
            access.upgrade_to_read()
        } else {
            access
        })
    }

    /// Revoke the currently-held story lock, if any.
    pub fn unlock(&self, conn: &GenericConnection) -> FictResult<()> {
        let update = try!(conn.prepare("
            UPDATE stories
            SET
                lock_user_id = NULL,
                lock_expiration = NULL
            WHERE
                id = $1 AND lock_user_id = $2
        "));

        let count = try!(update.execute(&[&self.id, &self.lock_user_id]));

        if count == 1 {
            Ok(())
        } else {
            Err(fict_err("Unable to revoke lock"))
        }
    }

    /// Persist any local changes into the database other than to the `lock_user_id` or
    /// `lock_expiration` fields.
    pub fn save(&self, conn: &GenericConnection) -> FictResult<()> {
        let update = try!(conn.prepare("
            UPDATE stories
            SET
                title = $2,
                published = $3,
                world_readable = $4,
                lock_duration_s = $5,
                contribution_count = $6,
                creation_time = $7,
                update_time = $8,
                publish_time = $9
            WHERE id = $1
        "));

        let count = try!(update.execute(&[
            &self.id,
            &self.title, &self.published, &self.world_readable, &self.lock_duration_s,
            &self.contribution_count,
            &self.creation_time, &self.update_time, &self.publish_time
        ]));

        if count == 1 {
            Ok(())
        } else {
            Err(fict_err("Unable to update story"))
        }
    }
}

/// Level of access granted to a specific `User` on a `Story`.
pub enum AccessLevel {
    NoAccess,
    Reader,
    Writer,
    Owner
}

impl AccessLevel {

    /// Convert an AccessLevel into an integer for serialization within a database table.
    fn encode(&self) -> i32 {
        match *self {
            AccessLevel::NoAccess => 0,
            AccessLevel::Reader => 1,
            AccessLevel::Writer => 2,
            AccessLevel::Owner => 3
        }
    }

    /// Create an AccessLevel from an integer previously encoded with `::encode()`.
    fn decode(value: i32) -> FictResult<AccessLevel> {
        match value {
            0 => Ok(AccessLevel::NoAccess),
            1 => Ok(AccessLevel::Reader),
            2 => Ok(AccessLevel::Writer),
            3 => Ok(AccessLevel::Owner),
            _ => Err(fict_err(format!("Invalid encoded access level [{}]", value)))
        }
    }

    /// Return true if this level permits users to know the existence of this `Story` in search
    /// results and so on.
    pub fn grants_read(&self) -> bool {
        match *self {
            AccessLevel::Reader | AccessLevel::Writer | AccessLevel::Owner => true,
            _ => false
        }
    }

    /// Return true if this level allows a user to contribute `Snippets` to this `Story`.
    pub fn grants_write(&self) -> bool {
        match *self {
            AccessLevel::Writer | AccessLevel::Owner => true,
            _ => false
        }
    }

    /// Return true if a user with this level should be able to grant and revoke access to other
    /// `Users`, determine when the `Story` is published, set or modify the title, or delete the
    /// story entirely.
    pub fn grants_admin(&self) -> bool {
        match *self {
            AccessLevel::Owner => true,
            _ => false
        }
    }

    /// Return a new AccessLevel that grants at least Reader access, but preserves higher access
    /// levels if granted.
    pub fn upgrade_to_read(self) -> AccessLevel {
        match self {
            AccessLevel::NoAccess => AccessLevel::Reader,
            _ => self
        }
    }

}

/// Associates a level of access to a `Story` with a `User`.
pub struct StoryAccess;

impl StoryAccess {

    /// Initialize database tables and indices used to store `StoryAccess` objects.
    ///
    /// Depends on `Story::initialize` and `User::initialize`.
    pub fn initialize(conn: &GenericConnection) -> FictResult<()> {
        try!(conn.execute("
            CREATE TABLE IF NOT EXISTS story_access (
                id BIGSERIAL PRIMARY KEY,
                story_id BIGINT NOT NULL REFERENCES stories (id)
                    ON DELETE CASCADE
                    ON UPDATE CASCADE,
                user_id BIGINT NOT NULL REFERENCES users (id)
                    ON DELETE CASCADE
                    ON UPDATE CASCADE,
                access_level_code INT NOT NULL,
                UNIQUE (user_id, story_id)
            )
        ", &[]));

        try!(conn.execute("
            CREATE INDEX IF NOT EXISTS story_access_story_id_index
            ON story_access (story_id, user_id)
        ", &[]));

        Ok(())
    }

    /// Grant a `User` access to a `Story` at a specified level. If level is `NoAccess`, any
    /// access will be removed.
    pub fn grant(conn: &GenericConnection, story: &Story, user: &User, level: &AccessLevel) -> FictResult<()> {
        match *level {
            AccessLevel::NoAccess => {
                // Revoke any existing access instead.
                let deletion = try!(conn.prepare("
                    DELETE FROM story_access
                    WHERE story_id = $1 AND user_id = $2
                "));

                try!(deletion.execute(&[&story.id, &user.id]));

                return Ok(());
            },
            _ => ()
        }

        let update = try!(conn.prepare("
            UPDATE story_access
            SET access_level_code = $1
            WHERE story_id = $2 AND user_id = $3
        "));

        let access_level_code = level.encode();

        let count = try!(update.execute(&[&access_level_code, &story.id, &user.id]));

        if count >= 1 {
            return Ok(())
        }

        // No existing access. Insert a new row, instead.
        let insertion = try!(conn.prepare("
            INSERT INTO story_access (access_level_code, story_id, user_id)
            VALUES ($1, $2, $3)
        "));
        try!(insertion.execute(&[&access_level_code, &story.id, &user.id]));

        Ok(())
    }

    /// Determine the current access level that a `User` has on a `Story`.
    fn access_for(conn: &GenericConnection, user: &User, story: &Story) -> FictResult<AccessLevel> {
        let locate = try!(conn.prepare("
            SELECT access_level_code
            FROM story_access
            WHERE user_id = $1 AND story_id = $2
        "));

        let rows = try!(locate.query(&[&user.id, &story.id]));
        let row_opt = try!(first_opt(&rows));

        row_opt
            .map(|row| AccessLevel::decode(row.get(0)))
            .unwrap_or(Ok(AccessLevel::NoAccess))
    }

}

/// Record when a User acquires a lock against a Story to prevent consecutive Snippets being
/// contributed by the same User.
pub struct ContributionAttempt;

impl ContributionAttempt {

    pub fn initialize(conn: &GenericConnection) -> FictResult<()> {
        try!(conn.execute("
            CREATE TABLE IF NOT EXISTS contribution_attempts (
                id BIGSERIAL PRIMARY KEY,
                story_id BIGINT NOT NULL REFERENCES stories (id)
                    ON DELETE CASCADE
                    ON UPDATE CASCADE,
                user_id BIGINT NOT NULL REFERENCES users (id)
                    ON DELETE CASCADE
                    ON UPDATE CASCADE,
                contribution_count INT NOT NULL,
                UNIQUE (user_id, story_id)
            )
        ", &[]));

        try!(conn.execute("
            CREATE INDEX IF NOT EXISTS contribution_attempts_story_id_index
            ON contribution_attempts (story_id)
        ", &[]));

        try!(conn.execute("
            CREATE INDEX IF NOT EXISTS contribution_attempts_user_id_index
            ON contribution_attempts (user_id)
        ", &[]));

        Ok(())
    }

    fn most_recent_attempt(conn: &GenericConnection, story: &Story, user: &User) -> FictResult<Option<i32>> {
        let select = try!(conn.prepare("
            SELECT contribution_count
            FROM contribution_attempts
            WHERE story_id = $1 AND user_id = $1
        "));

        let rows = try!(select.query(&[&story.id, &user.id]));
        let row_opt = try!(first_opt(&rows));

        Ok(row_opt.map(|row| row.get(0)))
    }

    /// Record a new contribution attempt.
    pub fn record(conn: &GenericConnection, story: &Story, user: &User) -> FictResult<()> {
        let update = try!(conn.prepare("
            UPDATE contribution_attempts
            SET contribution_count = $1
            WHERE story_id = $2 AND user_id = $3
        "));

        let count = try!(update.execute(&[&story.contribution_count, &story.id, &user.id]));

        if count >= 1 {
            return Ok(());
        }

        let insert = try!(conn.prepare("
            INSERT INTO contribution_attempts (contribution_count, story_id, user_id)
            VALUES ($1, $2, $3)
        "));
        try!(insert.execute(&[&story.contribution_count, &story.id, &user.id]));

        Ok(())
    }

}
