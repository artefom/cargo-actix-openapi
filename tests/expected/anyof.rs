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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum GreetUserBody {
    #[serde(rename = "First variant")]
    FirstVariant(Variant1),
    #[serde(rename = "Second variant")]
    SecondVariant(Variant2),
}

// Struct
// -------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GreetUserPath {
    /// The name of the user to greet.
    pub user: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Variant1 {
    pub foo: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Variant2 {
    pub bar: String,
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

// Api service
// -------------------------------

#[async_trait(?Send)]
pub trait ApiService<S>
where
    S: Send + Sync + 'static,
{
    /// Returns a greeting to the user!
    async fn greet_user(
        data: web::Data<S>,
        path: web::Path<GreetUserPath>,
        body: web::Json<GreetUserBody>,
    ) -> web::Json<String>;
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
            .route("/hello/{user}", post().to(T::greet_user))
            .route("/v1/hello/{user}", post().to(T::greet_user))
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
