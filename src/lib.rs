#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

pub use rusqlite;
pub use rusqlite_from_row_derive::FromRow;

/// A trait that allows mapping rusqlite rows to other types.
pub trait FromRow: Sized {
    /// Performce the conversion.
    ///
    /// # Panics
    ///
    /// panics if the row does not contain the expected column names.
    fn from_row(row: &rusqlite::Row) -> Self {
        Self::from_row_prefixed(row, "")
    }

    /// Try's to perform the conversion.
    ///
    /// Will return an error if the row does not contain the expected column names.
    fn try_from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Self::try_from_row_prefixed(row, "")
    }

    /// Perform the conversion. Each row will be extracted using it's name prefixed with
    /// `prefix`.
    ///
    /// # Panics
    ///
    /// panics if the row does not contain the expected column names.
    fn from_row_prefixed(row: &rusqlite::Row, prefix: &str) -> Self;

    /// Try's to perform the conversion. Each row will be extracted using it's name prefixed with
    /// `prefix`.
    ///
    /// Will return an error if the row does not contain the expected column names.
    fn try_from_row_prefixed(row: &rusqlite::Row, prefix: &str) -> Result<Self, rusqlite::Error>;
}
