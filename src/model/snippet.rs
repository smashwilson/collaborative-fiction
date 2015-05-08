use std::fmt::{self, Display, Formatter};

use postgres::Connection;
use time::Timespec;

use model::{create_index, first, Story};
use error::{FictResult, fict_err};

/// Single submission to an ongoing `Story`.
pub struct Snippet {
    pub id: Option<i64>,
    pub order: i32,
    pub user_id: i64,
    pub story_id: i64,
    pub creation_time: Timespec,
    pub content: String
}

impl Snippet {

    /// Initialize database tables and indices used to store `Snippet` objects.
    ///
    /// Depends on `Story::initialize` and `User::initialize`.
    pub fn initialize(conn: &Connection) -> FictResult<()> {
        try!(conn.execute("
            CREATE TABLE IF NOT EXISTS snippets (
                id BIGSERIAL PRIMARY KEY,
                order INT NOT NULL,
                user_id BIGINT REFERENCES users (id)
                    ON DELETE SET NULL
                    ON UPDATE CASCADE,
                story_id BIGINT REFERENCES stories (id)
                    ON DELETE CASCADE
                    ON UPDATE CASCADE,
                creation_time TIMESTAMP WITH TIME ZONE NOT NULL,
                content VARCHAR NOT NULL
            )
        ", &[]));

        try!(create_index(conn, "snippets_user_id_index",
            "CREATE INDEX snippets_user_id_index ON snippets (user_id)"
        ));

        try!(create_index(conn, "snippets_story_id_index",
            "CREATE INDEX snippets_story_id_index ON snippets (story_id)"
        ));

        Ok(())
    }

}
