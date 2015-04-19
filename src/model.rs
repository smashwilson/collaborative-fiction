//! Data model and PostgreSQL storage abstraction.

use std::borrow::ToOwned;

use postgres::{self, Connection};

use error::{FictError, FictResult, fict_err};

/// Participant in the collaborative storytelling process. Automatically created on first oauth
/// login.
pub struct User {
    id: Option<i64>,
    name: String,
    email: String,
}

/// Expect exactly zero or one results from a SQL query. Produce an error if more than one row was
/// returned.
fn first_opt(results: postgres::Rows) -> FictResult<Option<postgres::Row>> {
    let mut it = results.into_iter();
    let first = it.next();

    match it.next() {
        None => Ok(first),
        Some(_) => Err(fict_err("Expected only one result, but more than one were returned")),
    }
}

/// Execute a SQL statement that is expected to return exactly one result. Produces an
/// error if zero or more than one results are returned, or if the underlying query produces any.
fn first(results: postgres::Rows) -> FictResult<postgres::Row> {
    first_opt(results)
        .and_then(|r| r.ok_or(fict_err("Expected at least one result, but zero were returned")))
}

/// Create an index using the provided SQL if it doesn't already exist. This is a workaround for
/// IF NOT EXISTS not being available in PostgreSQL until 9.5.
fn create_index(conn: &Connection, name: &str, sql: &str) -> FictResult<()> {
    let existing_stmt = try!(conn.prepare(
        &format!("SELECT to_regclass('{}')::varchar", name)
    ));
    let existing_result = try!(existing_stmt.query(&[]));
    let row = try!(first(existing_result));
    let exists = match row.get_opt::<usize, String>(0) {
        Err(postgres::Error::WasNull) => false,
        Err(e) => return Err(From::from(e)),
        _ => true,
    };

    if ! exists {
        debug!("Creating index {}.", name);
        match conn.execute(sql, &[]) {
            Ok(_) => Ok(()),
            Err(e) => Err(From::from(e)),
        }
    } else {
        debug!("Index {} already exists.", name);
        Ok(())
    }
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
