// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Error handling and conversions.

use async_bb8_diesel::{ConnectionError, PoolError, PoolResult};
use diesel::result::DatabaseErrorInformation;
use diesel::result::DatabaseErrorKind as DieselErrorKind;
use diesel::result::Error as DieselError;
use omicron_common::api::external::{
    Error as PublicError, LookupType, ResourceType,
};

/// Wrapper around an error which may be returned from a Diesel transaction.
#[derive(Debug, thiserror::Error)]
pub enum TransactionError<T> {
    /// The customizable error type.
    ///
    /// This error should be used for all non-Diesel transaction failures.
    #[error("Custom transaction error; {0}")]
    CustomError(T),

    /// The Diesel error type.
    ///
    /// This error covers failure due to accessing the DB pool or errors
    /// propagated from the DB itself.
    #[error("Pool error: {0}")]
    Pool(#[from] async_bb8_diesel::PoolError),
}

// Maps a "diesel error" into a "pool error", which
// is already contained within the error type.
impl<T> From<DieselError> for TransactionError<T> {
    fn from(err: DieselError) -> Self {
        Self::Pool(PoolError::Connection(ConnectionError::Query(err)))
    }
}

/// Summarizes details provided with a database error.
fn format_database_error(
    kind: DieselErrorKind,
    info: &dyn DatabaseErrorInformation,
) -> String {
    let mut rv =
        format!("database error (kind = {:?}): {}\n", kind, info.message());
    if let Some(details) = info.details() {
        rv.push_str(&format!("DETAILS: {}\n", details));
    }
    if let Some(hint) = info.hint() {
        rv.push_str(&format!("HINT: {}\n", hint));
    }
    if let Some(table_name) = info.table_name() {
        rv.push_str(&format!("TABLE NAME: {}\n", table_name));
    }
    if let Some(column_name) = info.column_name() {
        rv.push_str(&format!("COLUMN NAME: {}\n", column_name));
    }
    if let Some(constraint_name) = info.constraint_name() {
        rv.push_str(&format!("CONSTRAINT NAME: {}\n", constraint_name));
    }
    if let Some(statement_position) = info.statement_position() {
        rv.push_str(&format!("STATEMENT POSITION: {}\n", statement_position));
    }
    rv
}

/// Like [`diesel::result::OptionalExtension<T>::optional`]. This turns Ok(v)
/// into Ok(Some(v)), Err("NotFound") into Ok(None), and leave all other values
/// unchanged.
pub fn diesel_pool_result_optional<T>(
    result: PoolResult<T>,
) -> PoolResult<Option<T>> {
    match result {
        Ok(v) => Ok(Some(v)),
        Err(PoolError::Connection(ConnectionError::Query(
            DieselError::NotFound,
        ))) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Allows the caller to handle user-facing errors, and provide additional
/// context which may be used to populate more informative errors.
///
/// Note that all operations may return server-level errors for a variety
/// of reasons, including being unable to contact the database, I/O errors,
/// etc.
pub enum ErrorHandler<'a> {
    /// The operation expected to fetch, update, or delete exactly one resource
    /// identified by the [`crate::authz::ApiResourceError`].
    /// If that row is not found, an appropriate "Not Found" error will be
    /// returned.
    NotFoundByResource(&'a dyn crate::authz::ApiResourceError),
    /// The operation was attempting to lookup or update a resource.
    /// If that row is not found, an appropriate "Not Found" error will be
    /// returned.
    ///
    /// NOTE: If you already have an [`crate::authz::ApiResource`] object, you
    /// should use the [`ErrorHandler::NotFoundByResource`] variant instead. Eventually,
    /// the only uses of this function should be in the DataStore functions
    /// that actually look up a record for the first time.
    NotFoundByLookup(ResourceType, LookupType),
    /// The operation was attempting to create a resource with a name.
    /// If a resource already exists with that name, an "Object Already Exists"
    /// error will be returned.
    Conflict(ResourceType, &'a str),
    /// The operation does not expect any user errors.
    /// Without additional context, all errors from this variant are translated
    /// to an "Internal Server Error".
    Server,
}

/// Converts a Diesel pool error to a public-facing error.
///
/// [`ErrorHandler`] may be used to add additional handlers for the error
/// being returned.
pub fn public_error_from_diesel_pool(
    error: PoolError,
    handler: ErrorHandler<'_>,
) -> PublicError {
    public_error_from_diesel_pool_helper(error, |error| match handler {
        ErrorHandler::NotFoundByResource(resource) => {
            public_error_from_diesel_lookup_helper(error, || {
                resource.not_found()
            })
        }
        ErrorHandler::NotFoundByLookup(resource_type, lookup_type) => {
            public_error_from_diesel_lookup_helper(error, || {
                PublicError::ObjectNotFound {
                    type_name: resource_type,
                    lookup_type,
                }
            })
        }
        ErrorHandler::Conflict(resource_type, object_name) => {
            public_error_from_diesel_create(error, resource_type, object_name)
        }
        ErrorHandler::Server => PublicError::internal_error(&format!(
            "unexpected database error: {:#}",
            error
        )),
    })
}

/// Handles the common cases for all pool errors (particularly around transient
/// errors while delegating the special case of
/// `PoolError::Connection(ConnectionError::Query(diesel_error))` to
/// `make_query_error(diesel_error)`, allowing the caller to decide how to
/// format a message for that case.
fn public_error_from_diesel_pool_helper<F>(
    error: PoolError,
    make_query_error: F,
) -> PublicError
where
    F: FnOnce(DieselError) -> PublicError,
{
    match error {
        PoolError::Connection(error) => match error {
            ConnectionError::Checkout(error) => PublicError::unavail(&format!(
                "Failed to access connection pool: {}",
                error
            )),
            ConnectionError::Query(error) => make_query_error(error),
        },
        PoolError::Timeout => {
            PublicError::unavail("Timeout accessing connection pool")
        }
    }
}

/// Converts a Diesel error to an external error, handling "NotFound" using
/// `make_not_found_error`.
fn public_error_from_diesel_lookup_helper<F>(
    error: DieselError,
    make_not_found_error: F,
) -> PublicError
where
    F: FnOnce() -> PublicError,
{
    match error {
        DieselError::NotFound => make_not_found_error(),
        DieselError::DatabaseError(kind, info) => {
            PublicError::internal_error(&format_database_error(kind, &*info))
        }
        error => PublicError::internal_error(&format!(
            "Unknown diesel error: {:?}",
            error
        )),
    }
}

/// Converts a Diesel error to an external error, when requested as
/// part of a creation operation.
fn public_error_from_diesel_create(
    error: DieselError,
    resource_type: ResourceType,
    object_name: &str,
) -> PublicError {
    match error {
        DieselError::DatabaseError(kind, info) => match kind {
            DieselErrorKind::UniqueViolation => {
                PublicError::ObjectAlreadyExists {
                    type_name: resource_type,
                    object_name: object_name.to_string(),
                }
            }
            _ => PublicError::internal_error(&format_database_error(
                kind, &*info,
            )),
        },
        _ => PublicError::internal_error(&format!(
            "Unknown diesel error: {:?}",
            error
        )),
    }
}
