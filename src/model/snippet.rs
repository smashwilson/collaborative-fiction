use postgres::GenericConnection;
use chrono::{DateTime, UTC};

use model::{first, Story, User};
use error::FictResult;

/// Single submission to an ongoing `Story`.
pub struct Snippet {
    pub id: i64,
    pub ordinal: i32,
    pub user_id: i64,
    pub story_id: i64,
    pub creation_time: DateTime<UTC>,
    pub content: String
}

impl Snippet {

    /// Initialize database tables and indices used to store `Snippet` objects.
    ///
    /// Depends on `Story::initialize` and `User::initialize`.
    pub fn initialize(conn: &GenericConnection) -> FictResult<()> {
        try!(conn.execute("
            CREATE TABLE IF NOT EXISTS snippets (
                id BIGSERIAL PRIMARY KEY,
                ordinal SERIAL NOT NULL,
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

        try!(conn.execute("
            CREATE INDEX IF NOT EXISTS snippets_user_id_index ON snippets (user_id)
        ", &[]));

        try!(conn.execute("
            CREATE INDEX IF NOT EXISTS snippets_story_id_index ON snippets (story_id)
        ", &[]));

        Ok(())
    }

    /// Accept data to construct a `Snippet` that begins a new `Story` in draft status.
    pub fn begin(conn: &GenericConnection, owner: &User, content: String) -> FictResult<(Snippet, Story)> {
        let story = try!(Story::begin(conn, owner));

        Snippet::contribute(conn, &story, owner, content)
            .map(|snippet| (snippet, story))
    }

    /// Continue a `Story` in progress by creating a new `Snippet`.
    pub fn contribute(conn: &GenericConnection, story: &Story, contributor: &User, content: String) -> FictResult<Snippet> {
        let contributor_id = contributor.id.unwrap();

        let insertion = try!(conn.prepare("
            INSERT INTO snippets (user_id, story_id, content)
            VALUES ($1, $2, $3)
            RETURNING id, ordinal, creation_time
        "));

        let rows = try!(insertion.query(&[&contributor_id, &story.id, &content]));
        let row = try!(first(&rows));

        Ok(Snippet{
            id: row.get(0),
            ordinal: row.get(1),
            user_id: contributor_id,
            story_id: story.id,
            creation_time: row.get(2),
            content: content
        })
    }

    /// Return the most recent Snippet associated with a given Story.
    pub fn most_recent(conn: &GenericConnection, story: &Story) -> FictResult<Snippet> {
        let selection = try!(conn.prepare("
            SELECT id, ordinal, user_id, story_id, creation_time, content
            FROM snippets
            WHERE story_id = $1
            ORDER BY ordinal DESC
            LIMIT 1
        "));

        let rows = try!(selection.query(&[&story.id]));
        let row = try!(first(&rows));

        Ok(Snippet{
            id: row.get(0),
            ordinal: row.get(1),
            user_id: row.get(2),
            story_id: row.get(3),
            creation_time: row.get(4),
            content: row.get(5)
        })
    }

}
