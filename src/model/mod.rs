//! Data model and PostgreSQL storage abstraction.

use std::env;
use std::default::Default;

use iron::Chain;
use iron::typemap::Key;
use persistent::Write;
use postgres::{self, Connection, SslMode};
use r2d2::{LoggingErrorHandler, Pool};
use r2d2_postgres::PostgresConnectionManager;

use error::{FictResult, fict_err};

mod user;

pub use self::user::User;

pub struct Session;

/// Database is the type key used to access the connection pool.
pub struct Database;

pub type PostgresPool = Pool<PostgresConnectionManager>;

impl Key for Database {
    type Value = PostgresPool;
}

impl Database {
    pub fn link(chain: &mut Chain) -> FictResult<()> {
        let pg_address = env::var("FICTION_PG").unwrap();

        let config = Default::default();
        let manager = PostgresConnectionManager::new(&*pg_address, SslMode::None).unwrap();
        let error_handler = Box::new(LoggingErrorHandler);
        let pool = Pool::new(config, manager, error_handler).unwrap();

        try!(Database::initialize(&pool));

        let w = Write::<Database>::one(pool);
        chain.link_before(w);

        Ok(())
    }

    fn initialize(pool: &PostgresPool) -> FictResult<()> {
        let conn = pool.get().unwrap();

        try!(User::initialize(&conn));

        Ok(())
    }
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
