//! A collaborative fiction user.

use postgres::Connection;

use model::create_index;
use error::FictResult;

/// Participant in the collaborative storytelling process. Automatically created on first oauth
/// login.
pub struct User {
    pub id: Option<i64>,
    pub name: String,
    pub email: String,
}

impl User {
    /// Create the database table used to store `User` instances. Do nothing if it already
    /// exists.
    pub fn initialize(conn: &Connection) -> FictResult<()> {
        try!(conn.execute("CREATE TABLE IF NOT EXISTS users (
            id BIGSERIAL PRIMARY KEY,
            name VARCHAR NOT NULL,
            email VARCHAR NOT NULL
        )", &[]));

        try!(create_index(conn, "email_index", "CREATE UNIQUE INDEX email_index ON users (email)"));

        Ok(())
    }

    /// Persist any local modifications to this `User` to the database.
    pub fn save(&mut self, conn: &Connection) -> FictResult<()> {
        match self.id {
            Some(existing_id) => {
                try!(conn.execute("
                    UPDATE users
                    SET name = $1, email = $2
                    WHERE id = $3
                ", &[&self.name, &self.email, &existing_id]));
                Ok(())
            },
            None => {
                let insertion = try!(conn.prepare("
                    INSERT INTO users (name, email)
                    VALUES ($1, $2)
                    RETURNING id
                "));
                let cursor = try!(insertion.query(&[&self.name, &self.email]));
                let row = cursor.into_iter().next().unwrap();
                self.id = Some(row.get(0));
                Ok(())
            },
        }
    }

    /// Discover an existing `User` by email address. If none exists, create, persist, and return a
    /// new one with the provided `name`.
    pub fn find_or_create(conn: &Connection, email: String, name: String) -> FictResult<User> {
        let selection = try!(conn.prepare("
            SELECT id, name, email FROM users
            WHERE email = $1
        "));
        let cursor = try!(selection.query(&[&email]));

        let user = match cursor.into_iter().next() {
            Some(row) => {
                User{
                    id: Some(row.get(0)),
                    name: row.get(1),
                    email: row.get(2),
                }
            },
            None => {
                let mut u = User{id: None, name: name, email: email};
                try!(u.save(conn));
                u
            }
        };

        Ok(user)
    }
}