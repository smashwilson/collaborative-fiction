use std::fmt::{self, Display, Formatter};

use postgres::Connection;
use time::Timespec;

use model::{create_index, first, Story, User};
use error::{FictResult, fict_err};

/// Single submission to an ongoing `Story`.
pub struct Snippet {
    pub id: i64,
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
                order SERIAL NOT NULL,
                user_id BIGINT REFERENCES users (id)
                    ON DELETE SET NULL
                    ON UPDATE CASCADE,
                story_id BIGINT REFERENCES stories (id)
                    ON DELETE CASCADE
                    ON UPDATE CASCADE,
                creation_time TIMESTAMP WITH TIME ZONE NOT NULL
                    DEFAULT (now() AT TIME ZONE 'utc'),
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

    /// Accept data to construct a `Snippet` that begins a new `Story` in draft status.
    pub fn begin(conn: &Connection, owner: &User, content: String) -> FictResult<Snippet> {
        let story = try!(Story::begin(conn, owner));

        Snippet::contribute(conn, &story, owner, content)
    }

    /// Continue a `Story` in progress by creating a new `Snippet`.
    pub fn contribute(conn: &Connection, story: &Story, contributor: &User, content: String) -> FictResult<Snippet> {
        let contributor_id = contributor.id.unwrap();

        let insertion = try!(conn.prepare("
            INSERT INTO snippets (user_id, story_id, content)
            VALUES ($1, $2, $3)
            RETURNING (id, order, creation_time)
        "));

        let rows = try!(insertion.query(&[&contributor_id, &story.id, &content]));
        let row = try!(first(rows));

        Ok(Snippet{
            id: row.get(0),
            order: row.get(1),
            user_id: contributor_id,
            story_id: story.id,
            creation_time: row.get(2),
            content: content
        })
    }

}
