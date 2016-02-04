//! Data model and PostgreSQL storage abstraction.

use std::env;
use std::default::Default;

use iron::Chain;
use iron::typemap::Key;
use persistent::Write;
use postgres::{self, Connection};
use postgres::rows::{Rows, Row};
use r2d2::{LoggingErrorHandler, Pool};
use r2d2_postgres::{PostgresConnectionManager, SslMode};

use error::{FictResult, fict_err};

mod user;
mod session;
mod story;
mod snippet;

pub use self::user::User;
pub use self::session::Session;
pub use self::story::{Story, StoryAccess, AccessLevel};
pub use self::snippet::Snippet;

/// Database is the type key used to access the connection pool.
pub struct Database;

pub type PostgresPool = Pool<PostgresConnectionManager>;

impl Key for Database {
    type Value = PostgresPool;
}

impl Database {
    pub fn link(chain: &mut Chain) -> FictResult<()> {
        let pg_address = try!(env::var("FICTION_PG"));

        let config = Default::default();
        let manager = try!(PostgresConnectionManager::new(&*pg_address, SslMode::None));
        let pool = try!(Pool::new(config, manager));

        try!(Database::initialize(&pool));

        let w = Write::<Database>::one(pool);
        chain.link_before(w);

        Ok(())
    }

    fn initialize(pool: &PostgresPool) -> FictResult<()> {
        let conn = try!(pool.get());

        // Reminder to self: this order is not arbitary. It must be organized such that foreign
        // keys are applied after the table they reference is created.
        try!(User::initialize(&conn));
        try!(Session::initialize(&conn));
        try!(Story::initialize(&conn));
        try!(StoryAccess::initialize(&conn));
        try!(Snippet::initialize(&conn));

        Ok(())
    }
}


/// Expect exactly zero or one results from a SQL query. Produce an error if more than one row was
/// returned.
fn first_opt<'a>(results: &'a Rows) -> FictResult<Option<Row<'a>>> {
    let mut it = results.iter();
    let first = it.next();

    match it.next() {
        None => Ok(first),
        Some(_) => Err(fict_err("Expected only one result, but more than one were returned")),
    }
}

/// Execute a SQL statement that is expected to return exactly one result. Produces an
/// error if zero or more than one results are returned, or if the underlying query produces any.
fn first<'a>(results: &'a Rows) -> FictResult<Row<'a>> {
    first_opt(results)
        .and_then(|r| r.ok_or(fict_err("Expected at least one result, but zero were returned")))
}
