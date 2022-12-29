//! API auto-generated by apigen

use std::fmt::Display;

use std::{collections::HashMap, fmt::Debug};

use serde::{Deserialize, Serialize};

use actix_web::{
    get, http::StatusCode, middleware::NormalizePath, web, App, HttpRequest, HttpResponse,
    HttpServer, ResponseError,
};

use async_trait::async_trait;

// Defaults
// -------------------------------
fn default_str_world() -> String {
    "World".to_string()
}
fn default_float_1() -> f64 {
    1
}
fn default_int_1() -> i64 {
    1
}
fn opt_default_float_1() -> Option<f64> {
    Some(1)
}
fn opt_default_int_1() -> Option<i64> {
    Some(1)
}

// Enums
// -------------------------------

// Struct
// -------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct GreetUserPath {
    /// The name of the user to greet.
    #[serde(default = "default_str_world")]
    pub user: String,
    #[serde(rename = "v1_float", default = "default_float_1")]
    pub v_1_float: f64,
    #[serde(rename = "v1_int", default = "default_int_1")]
    pub v_1_int: i64,
    #[serde(rename = "v1_opt_float", default = "opt_default_float_1")]
    pub v_1_opt_float: Option<f64>,
    #[serde(rename = "v1_opt_int", default = "opt_default_int_1")]
    pub v_1_opt_int: Option<i64>,
    #[serde(rename = "n1", default = "opt_default_int_1")]
    pub n_1: Option<i64>,
    #[serde(rename = "n2")]
    pub n_2: i64,
    #[serde(rename = "n3")]
    pub n_3: Option<i64>,
    #[serde(rename = "n4", default = "default_int_1")]
    pub n_4: i64,
}

// Error with details
// -------------------------------

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
    ) -> web::Json<String>;
}

// Run service function (+ helper functions)
// -----------------------------------------

static OPENAPI_FILE: &'static str = include_str!("static/openapi.yaml");
static DOCS_PAGE: &'static str = include_str!("static/docs.html");

/// Documentation
#[get("/openapi.yaml")]
async fn openapi() -> String {
    OPENAPI_FILE.to_string()
}

/// Documentation
#[get("/docs")]
async fn docs() -> HttpResponse {
    HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(DOCS_PAGE)
}

#[get("/")]
async fn redirect_to_docs() -> HttpResponse {
    HttpResponse::build(StatusCode::PERMANENT_REDIRECT)
        .append_header(("Location", "docs"))
        .body("")
}

pub async fn run_service<T, S>(bind: &str, initial_state: S) -> Result<(), std::io::Error>
where
    T: ApiService<S> + 'static,
    S: Send + Sync + 'static,
{
    let app_data = web::Data::new(initial_state);

    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .wrap(NormalizePath::trim())
            .service(openapi)
            .service(redirect_to_docs)
            .service(docs)
            .route(
                "/hello/{user}",
                web::get().to(T::greet_user)
            )
    })
    .bind(bind)?
    .run()
    .await?;

    Ok(())
}