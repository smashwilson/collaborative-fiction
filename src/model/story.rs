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
                id, title, published, world_readable, lock_duration_s,
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
            creation_time: row.get(5),
            update_time: row.get(6),
            publish_time: row.get(7),
            lock_user_id: row.get(8),
            lock_expiration: row.get(9)
        };

        // Automatically grant Owner access to the creating user.
        try!(StoryAccess::grant(conn, &story, owner, &AccessLevel::Owner));

        Ok(story)
    }

    /// Search for an existing `Story` by ID. If the story does not exist, or if the current user
    /// does not have sufficient access to write to this story, return `Err(FictError::NotFound)`.
    /// If the story is currently locked by someone else, return `Err(FictError::LockFailure)` with
    /// the lock details. Otherwise, return the locked `Story`.
    pub fn locked_for_write(conn: &Connection, id: i16, applicant: &User) -> FictResult<Story> {
        let now = UTC::now();
        let transaction = try!(conn.transaction());

        // Locate and lock the story row.
        let selection = try!(transaction.prepare("
            SELECT
                id, title, published, world_readable, lock_duration_s,
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
            creation_time: row.get(5),
            update_time: row.get(6),
            publish_time: row.get(7),
            lock_user_id: row.get(8),
            lock_expiration: row.get(9)
        });

        // Story ID does not match a known story.
        if story_opt.is_none() {
            return Err(FictError::NotFound);
        }
        let mut story = story_opt.unwrap();

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

        // Acquire the story lock.
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

        // Return the locked story.
        try!(transaction.commit());

        Ok(story)
    }

    /// Search for an existing `Story` by ID.
    pub fn with_id(conn: &GenericConnection, id: i64) -> FictResult<Option<Story>> {
        let selection = try!(conn.prepare("
            SELECT
                id, title, published, world_readable, lock_duration_s,
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
                creation_time: row.get(5),
                update_time: row.get(6),
                publish_time: row.get(7),
                lock_user_id: row.get(8),
                lock_expiration: row.get(9)
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
pub struct StoryAccess {
    pub id: i64,
    pub story_id: i64,
    pub user_id: i64,
    pub access_level: AccessLevel
}

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
            CREATE INDEX IF NOT EXISTS story_access_story_id_index ON story_access (story_id)
        ", &[]));

        try!(conn.execute("
            CREATE INDEX IF NOT EXISTS story_access_user_id_index ON story_access (user_id)
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
