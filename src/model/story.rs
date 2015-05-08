use std::fmt::{self, Display, Formatter};

use postgres::Connection;
use time::Timespec;

use model::{create_index, first};
use error::{FictResult, fict_err};

/// An ordered sequence of Snippets that combine to form a (hopefully) hilarious piece of fiction.
pub struct Story {
    pub id: Option<i64>,
    pub title: Option<String>,
    pub published: bool,
    pub world_readable: bool,
    pub creation_time: Timespec,
    pub update_time: Timespec,
    pub publish_time: Option<Timespec>,
    pub lock_user_id: Option<i64>,
    pub lock_expiration: Option<Timespec>
}

impl Story {

    /// Initialize database tables and indices used to story `Story` objects.
    ///
    /// Depends on `User::initialize`.
    pub fn initialize(conn: &Connection) -> FictResult<()> {
        try!(conn.execute("
            CREATE TABLE IF NOT EXISTS stories (
                id BIGSERIAL PRIMARY KEY,
                title VARCHAR,
                published BOOLEAN NOT NULL,
                world_readable BOOLEAN NOT NULL,
                creation_time TIMESTAMP WITH TIME ZONE NOT NULL,
                update_time TIMESTAMP WITH TIME ZONE NOT NULL,
                publish_time TIMESTAMP WITH TIME ZONE,
                lock_user_id BIGINT REFERENCES users (id)
                    ON DELETE SET NULL
                    ON UPDATE CASCADE,
                lock_expiration TIMESTAMP WITH TIME ZONE,
            )
        ", &[]));

        try!(create_index(conn, "stories_lock_index",
            "CREATE INDEX stories_lock_index ON stories (lock_user_id)"
        ));

        Ok(())
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

}

/// Associates a level of access to a `Story` with a `User`.
pub struct StoryAccess {
    pub id: Option<i64>,
    pub story_id: i64,
    pub user_id: i64,
    pub access_level: AccessLevel
}

impl StoryAccess {

    /// Initialize database tables and indices used to store `StoryAccess` objects.
    ///
    /// Depends on `Story::initialize` and `User::initialize`.
    pub fn initialize(conn: &Connection) -> FictResult<()> {
        try!(conn.execute("
            CREATE TABLE IF NOT EXISTS story_access (
                id BIGSERIAL PRIMARY KEY,
                story_id BIGINT NOT NULL REFERENCES stories (id)
                    ON DELETE CASCADE
                    ON UPDATE CASCADE,
                user_id BIGINT NOT NULL REFERENCES users (id)
                    ON DELETE CASCADE
                    ON UPDATE CASCADE,
            )
        ", &[]));

        try!(create_index(conn, "story_access_story_id_index",
            "CREATE INDEX story_access_story_id_index ON story_access (story_id)"
        ));

        try!(create_index(conn, "story_access_user_id_index",
            "CREATE INDEX story_access_user_id_index ON story_access (user_id)"
        ));

        Ok(())
    }

}
