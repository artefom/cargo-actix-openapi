#![allow(unused_imports)]

//! API auto-generated by apigen

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use serde::{Deserialize, Serialize};

use actix_web::{
    http::StatusCode,
    middleware::{NormalizePath, TrailingSlash},
    web, App, HttpRequest, HttpResponse, HttpServer, ResponseError,
};

use actix_web_prom::PrometheusMetricsBuilder;

use async_trait::async_trait;

// Defaults
// -------------------------------

// Enums
// -------------------------------

// Struct
// -------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct QuotaDetailsPath {
    /// Quota label - Unique quota identifier
    pub quota: String,
}

/// Quota specification
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Quota {
    /// The 'weight' of a single cell in milliseconds or emission interval.
    /// Maximum allowed requests per minute can be calculated as: 60 * 1000 / replanish_interval
    /// Controls sustainable Cell Rate
    pub replanish_interval: f64,
    /// Number of sequential cells allowed in a single burst
    /// A burst or clump of packets can arrive at a higher rate than determined by the emission interval
    /// In case there is unused burst capacity, quota can also exceed RPM in certain time frames.
    /// Burst capacity of 0 ensure that RPM is never exceeded but introduces a lot of delay.
    /// Burst capacity does not affect Sustainable Cell Rate
    pub burst_capacity: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MatchRule {
}

/// State information of the quota
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct QuotaState {
    /// Earliest delay in ms from now when next cell is available
    pub earliest_next_available: f64,
    /// Current remaining burst capacity
    pub remaining_burst_capacity: i64,
}

/// Quota statistics, purely descriptive. Not used in Rate limiting decisions.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct QuotaStats {
    /// Number of requests in last 60 seconds
    pub rpm: i64,
}

/// Full information about quota
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct QuotaDetails {
    /// Quota specification
    pub quota: Quota,
    /// Collection of predicates to test agains incomming queries
    /// If at least one predicate is matching the incomming query, the rate limit is applied to the request
    /// Multiple rate limits can be applied to incomming request at once
    #[serde(rename = "match")]
    pub match_: Vec<MatchRule>,
    /// State information of the quota
    pub state: QuotaState,
    /// Quota statistics, purely descriptive. Not used in Rate limiting decisions.
    pub stats: QuotaStats,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CellTestQuery {
    /// Query that will be matched against quotas
    ///
    /// # Matching rules
    /// ---------------
    ///
    /// Quota matches the query if at least one of it's predicates (match section) matches the query.
    /// Predicate matches query if all its key/values are present and match key/values of the request query.
    /// If query key is not present in the predicate, it is disregarded.
    ///
    /// ## Example:
    ///
    /// given predicate:
    ///
    ///
    /// `
    /// {
    ///   'carrier': 'MEGB'
    ///   'endpoint': 'location'
    /// }
    /// `
    ///
    /// match results on queries:
    ///
    ///
    /// `?carrier=MEGB&endpoint=locations` - OK
    ///
    ///
    /// `?carrier=MEGB` - No match
    ///
    ///
    /// `?carrier=MEGB&endpoint=locations&sender=retailer-api` - OK
    ///
    ///
    /// `?carrier=MEGB&sender=retailer-api` - No match
    ///
    ///
    /// `?sender=retailer-api` - No match
    pub query: MatchRule,
}

/// Information about current cell state and matched quotas.
/// Matched quotas are computed based on query.
/// Info and state are computed dynamically based on matched quotas.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CellDetails {
    /// Matched quotas
    pub quotas: Vec<String>,
    /// Quota specification
    pub info: Quota,
    /// State information of the quota
    pub state: QuotaState,
}

/// Information about current cell state.
/// Info and state are computed dynamically based on matched quotas.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CellInfo {
    /// Quota specification
    pub info: Quota,
    /// State information of the quota
    pub state: QuotaState,
}

/// Result of the cell update. Allowed/Denied flag + cell info
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateResult {
    /// Indicates if request was allowed
    /// If request was allowed, rate limit state was already updated to accomodate
    /// this request.
    /// If request was rejected, rate limit was not updated
    pub allowed: bool,
    /// Information about current cell state.
    /// Info and state are computed dynamically based on matched quotas.
    pub details: CellInfo,
}

// Error with details
// -------------------------------

/// Create detailed errors with ease
#[macro_export]
macro_rules! detailed {
    ($err:expr,$msg:expr) => {
        $crate::server::api::Detailed {
            error: $err,
            details: $msg.to_string(),
        }
    };
}

/// Bails with detailed api error
#[macro_export]
macro_rules! apibail {
    ($err:expr,$msg:expr) => {
        return Err($crate::server::api::Detailed {
            error: $err,
            details: $msg.to_string(),
        })
    };
}

pub trait StatusCoded {
    fn status_code(&self) -> StatusCode;
}

#[derive(Debug)]
pub struct Detailed<E> {
    pub error: E,
    pub details: String,
}

impl<E: Display> Display for Detailed<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}. Reason: {}", self.error, self.details)
    }
}

impl<E: Display + Debug> std::error::Error for Detailed<E> {}

impl<E: Display + Debug> ResponseError for Detailed<E>
where
    E: StatusCoded,
{
    fn status_code(&self) -> StatusCode {
        self.error.status_code()
    }
}

/// Converts some result to Result with detailed api error
pub trait ApiErr<T, E> {
    /// Wrap the error value with additional context.
    fn apierr<C>(self, err: C) -> Result<T, Detailed<C>>
    where
        C: Display + Send + Sync + 'static;
}

impl<T, E> ApiErr<T, E> for Result<T, E>
where
    E: Debug + Send + Sync + 'static,
{
    fn apierr<C>(self, err: C) -> Result<T, Detailed<C>>
    where
        C: Display + Send + Sync + 'static,
    {
        // Not using map_err to save 2 useless frames off the captured backtrace
        // in ext_context.
        match self {
            Ok(ok) => Ok(ok),
            Err(original_error) => Err(Detailed {
                error: err,
                details: format!("{:?}", original_error),
            }),
        }
    }
}

// Error
// -------------------------------

/// Status NOT_FOUND:
/// Quota not found
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum QuotaDetailsError {
    QuotaNotFound,
}

impl Display for QuotaDetailsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::QuotaNotFound => "Quota not found",
        };
        f.write_str(message)
    }
}

impl StatusCoded for QuotaDetailsError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::QuotaNotFound => StatusCode::NOT_FOUND,
        }
    }
}

/// Status BAD_REQUEST:
/// Duplicate key in query
///
/// Status NOT_FOUND:
/// No quotas matching given query found
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum CellTestError {
    DuplicateQueryKey,
    NoQuotasMatchingQueryFound,
}

impl Display for CellTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::DuplicateQueryKey => "Duplicate query key",
            Self::NoQuotasMatchingQueryFound => "No quotas matching query found",
        };
        f.write_str(message)
    }
}

impl StatusCoded for CellTestError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::DuplicateQueryKey => StatusCode::BAD_REQUEST,
            Self::NoQuotasMatchingQueryFound => StatusCode::NOT_FOUND,
        }
    }
}

// Api service
// -------------------------------

#[async_trait(?Send)]
pub trait ApiService<S>
where
    S: Send + Sync + 'static,
{
    /// Check service health
    async fn health(
        data: web::Data<S>,
    ) -> web::Json<String>;
    /// List quotas
    async fn quota_list(
        data: web::Data<S>,
    ) -> web::Json<Vec<String>>;
    /// Get quota details
    async fn quota_details(
        data: web::Data<S>,
        path: web::Path<QuotaDetailsPath>,
    ) -> Result<web::Json<QuotaDetails>,Detailed<QuotaDetailsError>>;
    /// Get current rate limitation state for given query
    async fn cell_test(
        data: web::Data<S>,
        query: web::Query<CellTestQuery>,
    ) -> Result<web::Json<CellDetails>,Detailed<CellTestError>>;
    /// Try to accomodate for one request
    async fn cell_update(
        data: web::Data<S>,
        query: web::Query<CellTestQuery>,
    ) -> Result<web::Json<UpdateResult>,Detailed<CellTestError>>;
}

// Run service function (+ helper functions)
// -----------------------------------------
static DOCS_OPENAPI: &str = include_str!("static/openapi.yaml");
static DOCS_HTML: &str = include_str!("static/docs.html");
async fn openapi() -> String {
    DOCS_OPENAPI.to_string()
}
async fn docs() -> HttpResponse {
    HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(DOCS_HTML)
}
async fn to_v1_docs() -> HttpResponse {
    HttpResponse::build(StatusCode::TEMPORARY_REDIRECT)
        .append_header(("Location", "v1/docs"))
        .body("")
}
async fn to_docs() -> HttpResponse {
    HttpResponse::build(StatusCode::TEMPORARY_REDIRECT)
        .append_header(("Location", "docs"))
        .body("")
}

// Tells that service is alive
async fn health() -> HttpResponse {
    HttpResponse::Ok().finish()
}

pub async fn run_service<T, S>(bind: &str, initial_state: S) -> Result<(), std::io::Error>
where
    T: ApiService<S> + 'static,
    S: Send + Sync + 'static,
{
    let app_data = web::Data::new(initial_state);

    let prometheus = PrometheusMetricsBuilder::new("api")
        .endpoint("/metrics")
        .build()
        .unwrap();

    use web::{delete, get, post};

    HttpServer::new(move || {

        let api = web::scope("")
            .wrap(prometheus.clone())
            .route("/cell/test", get().to(T::cell_test))
            .route("/cell/update", post().to(T::cell_update))
            .route("/health", get().to(T::health))
            .route("/quota", get().to(T::quota_list))
            .route("/quota/{quota}", get().to(T::quota_details))
            .route("/v1/cell/test", get().to(T::cell_test))
            .route("/v1/cell/update", post().to(T::cell_update))
            .route("/v1/health", get().to(T::health))
            .route("/v1/quota", get().to(T::quota_list))
            .route("/v1/quota/{quota}", get().to(T::quota_details))
            .wrap(prometheus.clone());

        App::new()
            .app_data(app_data.clone())
            .wrap(NormalizePath::new(TrailingSlash::MergeOnly))
            // Aux services
            .route("/health", get().to(health))
            // Static paths
            .route("/", get().to(to_docs))
            .route("/docs", get().to(docs))
            .route("/openapi.yaml", get().to(openapi))
            .route("/v1", get().to(to_v1_docs))
            .route("/v1/", get().to(to_docs))
            .route("/v1/docs", get().to(docs))
            .route("/v1/openapi.yaml", get().to(openapi))
            // Server routes
            .service(api)
    })
    .bind(bind)?
    .run()
    .await?;

    Ok(())
}
