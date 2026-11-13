//! Checked bridge between unsigned domain counters and SQLite's signed INTEGER.
//! Corrupt negative values and counters outside the storage range fail closed.

use rusqlite::{Error, Row, types::Type};

pub(super) fn read_nonnegative(row: &Row<'_>, column: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(column)?;
    u64::try_from(value)
        .map_err(|error| Error::FromSqlConversionFailure(column, Type::Integer, Box::new(error)))
}

pub(super) fn write_nonnegative(value: u64) -> rusqlite::Result<i64> {
    i64::try_from(value).map_err(|error| Error::ToSqlConversionFailure(Box::new(error)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn domain_counters_roundtrip_at_sqlite_integer_boundaries() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute("CREATE TABLE counters (sequence INTEGER NOT NULL)", [])
            .unwrap();
        for value in [0, 31, i64::MAX as u64] {
            let encoded = write_nonnegative(value).unwrap();
            connection
                .execute("INSERT INTO counters VALUES (?1)", [encoded])
                .unwrap();
            let observed = connection
                .query_row(
                    "SELECT sequence FROM counters ORDER BY rowid DESC LIMIT 1",
                    [],
                    |row| read_nonnegative(row, 0),
                )
                .unwrap();
            assert_eq!(observed, value);
        }
        assert!(write_nonnegative(i64::MAX as u64 + 1).is_err());
        assert!(write_nonnegative(u64::MAX).is_err());
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM counters", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn negative_and_noninteger_database_values_cannot_become_unsigned_counters() {
        let connection = Connection::open_in_memory().unwrap();
        for query in ["SELECT -1", "SELECT 1.5", "SELECT '12'", "SELECT NULL"] {
            assert!(
                connection
                    .query_row(query, [], |row| read_nonnegative(row, 0))
                    .is_err()
            );
        }
        let bytes = connection
            .query_row("SELECT length(CAST('作品' AS BLOB))", [], |row| {
                read_nonnegative(row, 0)
            })
            .unwrap();
        assert_eq!(bytes, 6);
    }
}
