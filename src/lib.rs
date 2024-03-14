#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

pub use rusqlite;
pub use rusqlite_from_row_derive::FromRow;

/// A trait that allows mapping a [`rusqlite::Row`] to other types.
pub trait FromRow: Sized {
    /// Performs the conversion.
    ///
    /// # Panics
    ///
    /// Panics if the row does not contain the expected column names.
    fn from_row(row: &rusqlite::Row) -> Self {
        Self::from_row_prefixed(row, None)
    }

    /// Try's to perform the conversion.
    ///
    /// Will return an error if the row does not contain the expected column names.
    fn try_from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Self::try_from_row_prefixed(row, None)
    }

    /// Perform the conversion. Each row will be extracted using it's name prefixed with
    /// `prefix`.
    ///
    /// # Panics
    ///
    /// Panics if the row does not contain the expected column names.
    fn from_row_prefixed(row: &rusqlite::Row, prefix: Option<&str>) -> Self {
        Self::try_from_row_prefixed(row, prefix).expect("from row failed")
    }

    /// Try's to perform the conversion. Each row will be extracted using it's name prefixed with
    /// `prefix`.
    ///
    /// Will return an error if the row does not contain the expected column names.
    fn try_from_row_prefixed(
        row: &rusqlite::Row,
        prefix: Option<&str>,
    ) -> Result<Self, rusqlite::Error>;

    /// Try's to check if all the columns that are needed by this struct are sql 'null' values.
    ///
    /// Will return an error if the row does not contain the expected column names.
    fn is_all_null(row: &rusqlite::Row, prefix: Option<&str>) -> Result<bool, rusqlite::Error>;
}

impl<T: FromRow> FromRow for Option<T> {
    fn try_from_row_prefixed(
        row: &rusqlite::Row,
        prefix: Option<&str>,
    ) -> Result<Self, rusqlite::Error> {
        if T::is_all_null(row, prefix)? {
            Ok(None)
        } else {
            Ok(Some(T::try_from_row_prefixed(row, prefix)?))
        }
    }

    fn is_all_null(row: &rusqlite::Row, prefix: Option<&str>) -> Result<bool, rusqlite::Error> {
        T::is_all_null(row, prefix)
    }
}
