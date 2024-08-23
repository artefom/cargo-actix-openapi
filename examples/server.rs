mod api;

use crate::apibail;
use actix_web::{web, App, HttpServer};
use actix_web_prom::PrometheusMetricsBuilder;
use api::*;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RunServiceError {
    #[error("Prometheus error {0}")]
    Prometheus(String),
    #[error("Binding error {0}")]
    Bind(std::io::Error),
    #[error("Runtime error {0}")]
    Runtime(std::io::Error),
}

pub async fn run_server<S>(bind: &str, initial_state: S) -> Result<(), RunServiceError>
where
    S: ServerState + Send + Sync + 'static,
{
    run_service::<DefaultServer, S>(bind, initial_state).await
}

pub async fn run_service<T, S>(bind: &str, initial_state: S) -> Result<(), RunServiceError>
where
    T: ApiService<S> + 'static,
    S: Send + Sync + 'static,
{
    let app_data = web::Data::new(initial_state);

    let registry = prometheus::default_registry();

    let prometheus = PrometheusMetricsBuilder::new("api")
        .endpoint("/metrics")
        .registry_ref(registry)
        .build()
        .map_err(|e| RunServiceError::Prometheus(e.to_string()))?;

    HttpServer::new(move || {
        let cors = actix_cors::Cors::default()
            .allow_any_header()
            .allow_any_method()
            .allow_any_origin()
            .supports_credentials()
            .max_age(3600);

        let scope = make_scope::<T, S>(prometheus.clone());
        App::new()
            .app_data(app_data.clone())
            .wrap(cors)
            .service(scope)
    })
    .bind(bind)
    .map_err(RunServiceError::Bind)?
    .run()
    .await
    .map_err(RunServiceError::Runtime)?;

    Ok(())
}
#[async_trait(?Send)]
pub trait ServerState {
    async fn get_greeting(&self) -> &String;
}

struct DefaultServer;

#[async_trait(?Send)]
impl<S> api::ApiService<S> for DefaultServer
where
    S: ServerState + Send + Sync + 'static,
{
    /// Service Health check
    async fn health(_data: web::Data<S>) -> web::Json<HealthResponse> {
        return web::Json(HealthResponse::Ok);
    }

    async fn hello_user(
        data: web::Data<S>,
        path: web::Path<HelloUserPath>,
    ) -> Result<web::Json<String>, Detailed<HelloUserError>> {
        if !path
            .user
            .chars()
            .into_iter()
            .all(|x| x.is_ascii_alphanumeric())
        {
            apibail!(
                HelloUserError::InvalidCharacters,
                "Found non-ascii-alphanumeric characters"
            )
        };

        Ok(web::Json(format!(
            "{}, {}!",
            data.get_greeting().await,
            path.user
        )))
    }
}
