//! README §Storage → Engine — "one writer connection plus a pool of reader connections."

use crate::error::Result;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OpenFlags};
use secrecy::{ExposeSecret, SecretString};
use std::path::Path;
use std::sync::Arc;

/// Default reader pool size — clinic scale (a handful of workstations) needs few readers.
pub const DEFAULT_READER_POOL_SIZE: u32 = 8;

/// Build the reader pool. Each new connection is initialized with the SQLCipher
/// passphrase and the anamnez pragma set.
pub fn build_reader_pool(
    path: &Path,
    passphrase: SecretString,
    size: u32,
) -> Result<Pool<SqliteConnectionManager>> {
    let pass_for_init = Arc::new(passphrase);
    let pass_clone = Arc::clone(&pass_for_init);

    let manager = SqliteConnectionManager::file(path)
        .with_flags(
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_init(move |conn| {
            init_reader(conn, pass_clone.expose_secret()).map_err(rusqlite_io_error)
        });

    let pool = Pool::builder().max_size(size).build(manager).map_err(|e| {
        crate::error::Error::Invariant(string_leak(&format!("reader pool build: {e}")))
    })?;
    Ok(pool)
}

/// Open the single writer connection. Read-write, exclusive use behind a `Mutex`.
pub fn open_writer(path: &Path, passphrase: &SecretString) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    super::pragmas::apply(&conn, passphrase.expose_secret())?;
    Ok(conn)
}

fn init_reader(conn: &mut Connection, passphrase: &str) -> rusqlite::Result<()> {
    // Same pragma sequence as the writer; reader pool gets read-only flag at open time.
    super::pragmas::apply(conn, passphrase).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(format!(
            "reader init: {e}"
        ))))
    })
}

fn rusqlite_io_error(e: rusqlite::Error) -> rusqlite::Error {
    e
}

fn string_leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
