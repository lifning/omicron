/*!
 * Primary control plane interface for database read and write operations
 */

use super::Pool;
use crate::api_error::ApiError;
use crate::api_model::ApiIdentityMetadata;
use crate::api_model::ApiInstance;
use crate::api_model::ApiInstanceCreateParams;
use crate::api_model::ApiInstanceRuntimeState;
use crate::api_model::ApiName;
use crate::api_model::ApiProject;
use crate::api_model::ApiProjectCreateParams;
use crate::api_model::ApiProjectUpdateParams;
use crate::api_model::ApiResourceType;
use crate::api_model::CreateResult;
use crate::api_model::DataPageParams;
use crate::api_model::DeleteResult;
use crate::api_model::ListResult;
use crate::api_model::LookupResult;
use crate::api_model::UpdateResult;
use chrono::DateTime;
use chrono::Utc;
use futures::StreamExt;
use std::convert::TryFrom;
use std::future::Future;
use std::sync::Arc;
use tokio_postgres::row::Row;
use tokio_postgres::types::ToSql;
use uuid::Uuid;

pub struct DataStore {
    pool: Arc<Pool>,
}

impl DataStore {
    pub fn new(pool: Arc<Pool>) -> Self {
        DataStore { pool }
    }

    /// Create a project
    pub async fn project_create_with_id(
        &self,
        new_id: &Uuid,
        new_project: &ApiProjectCreateParams,
    ) -> CreateResult<ApiProject> {
        let client = self.pool.acquire().await?;
        let now = Utc::now();
        sql_insert_unique(
            &client,
            Project,
            Project::ALL_COLUMNS,
            &new_project.identity.name.as_str(),
            &[
                new_id,
                &new_project.identity.name,
                &new_project.identity.description,
                &now,
                &now,
                &(None as Option<DateTime<Utc>>),
            ],
        )
        .await
    }

    /// Fetch metadata for a project
    pub async fn project_fetch(
        &self,
        project_name: &ApiName,
    ) -> LookupResult<ApiProject> {
        let client = self.pool.acquire().await?;
        let rows = client
            .query(
                format!(
                    "SELECT {} FROM {} WHERE time_deleted IS NULL AND \
                        name = $1 LIMIT 2",
                    Project::ALL_COLUMNS.join(", "),
                    Project::TABLE_NAME
                )
                .as_str(),
                &[&project_name],
            )
            .await
            .map_err(sql_error)?;
        match rows.len() {
            1 => Ok(Arc::new(ApiProject::try_from(&rows[0])?)),
            0 => Err(ApiError::not_found_by_name(
                ApiResourceType::Project,
                project_name,
            )),
            len => Err(ApiError::internal_error(&format!(
                "expected at most one row from database query, but found {}",
                len
            ))),
        }
    }

    /// Delete a project
    pub async fn project_delete(&self, project_name: &ApiName) -> DeleteResult {
        /*
         * XXX TODO-correctness This needs to check whether the project has any
         * resources in it.  One way to do that would be to define a certain
         * kind of generation number, maybe called a "resource-creation
         * generation number" ("rcgen").  Every resource creation bumps the
         * "rgen" of the parent project.  This operation fetches the current
         * rgen of the project, then checks for various kinds of resources, then
         * does the following UPDATE that sets time_deleted _conditional_ on the
         * rgen being the same.  If this updates 0 rows, either the project is
         * gone already or a new resource has been created.  (We'll want to make
         * sure that we report the correct error!)  We'll want to think more
         * carefully about this scheme, and maybe alternatives.  (Another idea
         * would be to have resource creation and deletion update a regular
         * counter.  But isn't that the same as denormalizing this piece of
         * information?)
         *
         * Can we do all this in one big query, maybe with a few CTEs?  (e.g.,
         * something like:
         *
         * WITH project AS
         *     (select id from Project where time_deleted IS NULL and name =
         *     $1),
         *     project_instances as (select id from Instance where time_deleted
         *     IS NULL and project_id = project.id LIMIT 1),
         *     project_disks as (select id from Disk where time_deleted IS NULL
         *     and project_id = project.id LIMIT 1),
         *
         *     UPDATE Project set time_deleted = $1 WHERE time_deleted IS NULL
         *         AND id = project.id AND project_instances ...
         *
         * I'm not sure how to finish that SQL, and moreover, I'm not sure it
         * solves the problem.  You can still create a new instance after
         * listing the instances.  So I guess you still need the "rcgen".
         * Still, you could potentially combine these to do it all in one go:
         *
         * WITH project AS
         *     (select id,rcgen from Project where time_deleted IS NULL and
         *     name = $1),
         *     project_instances as (select id from Instance where time_deleted
         *     IS NULL and project_id = project.id LIMIT 1),
         *     project_disks as (select id from Disk where time_deleted IS NULL
         *     and project_id = project.id LIMIT 1),
         *
         *     UPDATE Project set time_deleted = $1 WHERE time_deleted IS NULL
         *         AND id = project.id AND rcgen = project.rcgen AND
         *         project_instances ...
         */
        let client = self.pool.acquire().await?;
        let now = Utc::now();
        let rows = client
            .query(
                format!(
                    "UPDATE {} SET time_deleted = $1 WHERE \
                    time_deleted IS NULL AND name = $2 LIMIT 2 \
                    RETURNING {}",
                    Project::TABLE_NAME,
                    Project::ALL_COLUMNS.join(", ")
                )
                .as_str(),
                &[&now, &project_name],
            )
            .await
            .map_err(sql_error)?;
        /* TODO-log log the returned row(s) */
        match rows.len() {
            1 => Ok(()),
            0 => Err(ApiError::not_found_by_name(
                ApiResourceType::Project,
                project_name,
            )),
            len => Err(ApiError::internal_error(&format!(
                "expected at most one row from database query, but found {}",
                len
            ))),
        }
    }

    /// Look up the id for a project based on its name
    pub async fn project_lookup_id_by_name(
        &self,
        name: &ApiName,
    ) -> Result<Uuid, ApiError> {
        let client = self.pool.acquire().await?;

        let rows = client
            .query(
                format!(
                    "SELECT id FROM {} WHERE name = $1 AND \
                        time_deleted IS NULL LIMIT 2",
                    Project::TABLE_NAME
                )
                .as_str(),
                &[&name],
            )
            .await
            .map_err(sql_error)?;
        match rows.len() {
            1 => Ok(rows[0].try_get(0).map_err(sql_error)?),
            0 => {
                Err(ApiError::not_found_by_name(ApiResourceType::Project, name))
            }
            /*
             * TODO-design This should really include a bunch more information,
             * like the SQL and the type of object we were expecting.
             */
            _ => Err(ApiError::internal_error(&format!(
                "expected 1 row from database query, but found {}",
                rows.len()
            ))),
        }
    }

    /// List a page of projects by id
    pub async fn projects_list_by_id(
        &self,
        pagparams: &DataPageParams<'_, Uuid>,
    ) -> ListResult<ApiProject> {
        let client = self.pool.acquire().await?;
        let rows = sql_pagination(
            &client,
            Project,
            Project::ALL_COLUMNS,
            "time_deleted IS NULL",
            "id",
            pagparams,
        )
        .await?;
        let list = rows
            .iter()
            .map(|row| ApiProject::try_from(row).map(Arc::new))
            .collect::<Vec<Result<Arc<ApiProject>, ApiError>>>();
        Ok(futures::stream::iter(list).boxed())
    }

    /// List a page of projects by name
    pub async fn projects_list_by_name(
        &self,
        pagparams: &DataPageParams<'_, ApiName>,
    ) -> ListResult<ApiProject> {
        let client = self.pool.acquire().await?;
        let rows = sql_pagination(
            &client,
            Project,
            Project::ALL_COLUMNS,
            "time_deleted IS NULL",
            "name",
            pagparams,
        )
        .await?;
        let list = rows
            .iter()
            .map(|row| ApiProject::try_from(row).map(Arc::new))
            .collect::<Vec<Result<Arc<ApiProject>, ApiError>>>();
        Ok(futures::stream::iter(list).boxed())
    }

    /// Updates a project by name (clobbering update -- no etag)
    pub async fn project_update(
        &self,
        project_name: &ApiName,
        update_params: &ApiProjectUpdateParams,
    ) -> UpdateResult<ApiProject> {
        let client = self.pool.acquire().await?;
        let now = Utc::now();

        // XXX handle name conflict error explicitly
        let mut sql = format!(
            "UPDATE {} SET time_metadata_updated = $1 ",
            Project::TABLE_NAME
        );
        let mut params: Vec<&(dyn ToSql + Sync)> = vec![&now];

        if let Some(new_name) = &update_params.identity.name {
            sql.push_str(&format!(", name = ${} ", params.len() + 1));
            params.push(new_name);
        }

        if let Some(new_description) = &update_params.identity.description {
            sql.push_str(&format!(", description = ${} ", params.len() + 1));
            params.push(new_description);
        }

        sql.push_str(&format!(
            " WHERE name = ${} AND time_deleted IS NULL LIMIT 2 RETURNING {}",
            params.len() + 1,
            Project::ALL_COLUMNS.join(", ")
        ));
        params.push(project_name);

        let rows =
            client.query(sql.as_str(), &params).await.map_err(sql_error)?;
        match rows.len() {
            0 => Err(ApiError::not_found_by_name(
                ApiResourceType::Project,
                project_name,
            )),
            1 => Ok(Arc::new(ApiProject::try_from(&rows[0])?)),
            len => Err(ApiError::internal_error(&format!(
                "expected 1 row from UPDATE query, but found {}",
                len
            ))),
        }
    }

    pub async fn project_create_instance(
        &self,
        instance_id: &Uuid,
        project_id: &Uuid,
        params: &ApiInstanceCreateParams,
        runtime_initial: &ApiInstanceRuntimeState,
    ) -> CreateResult<ApiInstance> {
        // XXX
        todo!();
    }

    pub async fn instance_lookup_by_id(
        &self,
        id: &Uuid,
    ) -> LookupResult<ApiInstance> {
        // XXX
        todo!();
    }

    pub async fn instance_update(
        &self,
        new_instance: Arc<ApiInstance>,
    ) -> Result<(), ApiError> {
        // XXX
        todo!();
    }
}

/* XXX Should this be From<>?  May want more context */
fn sql_error(e: tokio_postgres::Error) -> ApiError {
    match e.code() {
        None => ApiError::InternalError {
            message: format!("unexpected database error: {}", e.to_string()),
        },
        Some(code) => ApiError::InternalError {
            message: format!(
                "unexpected database error (code {}): {}",
                code.code(),
                e.to_string()
            ),
        },
    }
}

fn sql_error_create(
    rtype: ApiResourceType,
    unique_value: &str,
    e: tokio_postgres::Error,
) -> ApiError {
    if let Some(code) = e.code() {
        if *code == tokio_postgres::error::SqlState::UNIQUE_VIOLATION {
            return ApiError::ObjectAlreadyExists {
                type_name: rtype,
                object_name: unique_value.to_owned(),
            };
        }
    }

    sql_error(e)
}

impl TryFrom<&tokio_postgres::Row> for ApiProject {
    type Error = ApiError;

    fn try_from(value: &tokio_postgres::Row) -> Result<Self, Self::Error> {
        // XXX really need some kind of context for these errors
        let name_str: &str = value.try_get("name").map_err(sql_error)?;
        let name = ApiName::try_from(name_str).map_err(|e| {
            ApiError::internal_error(&format!(
                "database project.name {:?}: {}",
                name_str, e
            ))
        })?;
        // XXX What to do with non-NULL time_deleted?
        Ok(ApiProject {
            generation: 1, // XXX
            identity: ApiIdentityMetadata {
                id: value.try_get("id").map_err(sql_error)?,
                name,
                description: value.try_get("description").map_err(sql_error)?,
                time_created: value
                    .try_get("time_created")
                    .map_err(sql_error)?,
                // XXX is it time_updated or time_metadata_updated
                time_modified: value
                    .try_get("time_metadata_updated")
                    .map_err(sql_error)?,
            },
        })
    }
}

/*
 * XXX building SQL like this sucks (obviously).  The 'static str here is just
 * to make it less likely we accidentally put a user-provided string here if
 * this sicence experiment escapes the lab.
 * As below, the explicit desugaring of "async fn" here is due to
 * rust-lang/rust#63033.
 */
fn sql_pagination<'a, T: Table, K: ToSql + Send + Sync>(
    client: &'a tokio_postgres::Client,
    table: T,
    columns: &[&'static str],
    base_where: &'static str,
    column_name: &'static str,
    pagparams: &'a DataPageParams<'a, K>,
) -> impl Future<Output = Result<Vec<Row>, ApiError>> + 'a {
    let (operator, order) = match pagparams.direction {
        dropshot::PaginationOrder::Ascending => (">", "ASC"),
        dropshot::PaginationOrder::Descending => ("<", "DESC"),
    };

    let base_sql = format!(
        "SELECT {} FROM {} WHERE {} ",
        columns.join(", "),
        T::TABLE_NAME,
        base_where
    );
    let limit = i64::from(pagparams.limit.get());
    async move {
        let query_result = if let Some(marker_value) = &pagparams.marker {
            let sql = format!(
                "{} AND {} {} $1 ORDER BY {} {} LIMIT $2",
                base_sql, column_name, operator, column_name, order
            );
            client.query(sql.as_str(), &[&marker_value, &limit]).await
        } else {
            let sql = format!(
                "{} ORDER BY {} {} LIMIT $1",
                base_sql, column_name, order
            );
            client.query(sql.as_str(), &[&limit]).await
        };

        query_result.map_err(sql_error)
    }
}

impl ToSql for ApiName {
    fn to_sql(
        &self,
        ty: &tokio_postgres::types::Type,
        out: &mut tokio_postgres::types::private::BytesMut,
    ) -> Result<
        tokio_postgres::types::IsNull,
        Box<dyn std::error::Error + Sync + Send>,
    >
    where
        Self: Sized,
    {
        self.as_str().to_sql(ty, out)
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool
    where
        Self: Sized,
    {
        <&str as ToSql>::accepts(ty)
    }

    fn to_sql_checked(
        &self,
        ty: &tokio_postgres::types::Type,
        out: &mut tokio_postgres::types::private::BytesMut,
    ) -> Result<
        tokio_postgres::types::IsNull,
        Box<dyn std::error::Error + Sync + Send>,
    > {
        self.as_str().to_sql_checked(ty, out)
    }
}

/**
 * Using database connection `client`, insert a row into table `table_name`
 * having values `values` for the respective columns named `table_fields`.
 */
/*
 * This is not as statically type-safe an API as you might think by looking at
 * it.  There's nothing that ensures that the types of the values correspond to
 * the right columns.  It's worth noting, however, that even if we statically
 * checked this, we would only be checking that the values correspond with some
 * Rust representation of the database schema that we've built into this
 * program.  That does not eliminate the runtime possibility that the types do
 * not, in fact, match the types in the database.
 *
 * The use of `'static` lifetimes here is a cheesy sanity check to catch SQL
 * injection.  (This is not a _good_ way to avoid SQL injection.  This is
 * intended as a last-ditch sanity check in case this code survives longer than
 * expected.)  Using the `async fn` syntax here runs afoul of
 * rust-lang/rust#63033.  So we desugar the `async` explicitly.
 */
fn sql_insert_unique<'a, T>(
    client: &'a tokio_postgres::Client,
    table: T,
    table_columns: &'a [&'static str],
    unique_value: &'a str,
    values: &'a [&'a (dyn ToSql + Sync)],
) -> impl Future<Output = Result<Arc<T::ApiModelType>, ApiError>> + 'a
where
    T: Table,
{
    assert_eq!(table_columns.len(), values.len());
    // XXX Could assert that the specified columns are a subset of allowed ones
    let table_field_str = table_columns.join(", ");
    let all_columns_str = T::ALL_COLUMNS.join(", ");
    let values_str = (1..=values.len())
        .map(|i| format!("${}", i))
        .collect::<Vec<String>>()
        .as_slice()
        .join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) RETURNING {}",
        T::TABLE_NAME,
        table_field_str,
        values_str,
        all_columns_str
    );
    async move {
        let rows = client
            .query(sql.as_str(), values)
            .await
            .map_err(|e| sql_error_create(T::RESOURCE_TYPE, unique_value, e))?;
        let row = match rows.len() {
            1 => &rows[0],
            len => {
                return Err(ApiError::internal_error(&format!(
                    "expected 1 row from INSERT query, but found {}",
                    len
                )))
            }
        };

        // XXX With this design, do we really want to use Arc?
        Ok(Arc::new(T::ApiModelType::try_from(row)?))
    }
}

/*
 * We want to find a better way to abstract this.  Diesel provides a compelling
 * model in terms of using it, but it also seems fairly heavyweight, and this
 * fetch-or-insert all-fields-of-an-object likely _isn't_ our most common use
 * case, even though we do it a lot for basic CRUD.
 */
trait Table {
    type ApiModelType: for<'a> TryFrom<&'a Row, Error = ApiError>;
    const RESOURCE_TYPE: ApiResourceType;
    const TABLE_NAME: &'static str;
    const ALL_COLUMNS: &'static [&'static str];
}

struct Project;
impl Table for Project {
    type ApiModelType = ApiProject;
    const RESOURCE_TYPE: ApiResourceType = ApiResourceType::Project;
    const TABLE_NAME: &'static str = "Project";
    const ALL_COLUMNS: &'static [&'static str] = &[
        "id",
        "name",
        "description",
        "time_created",
        "time_metadata_updated",
        "time_deleted",
    ];
}
