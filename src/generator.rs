use openapiv3::OpenAPI;
use serde::ser;
use serde_yaml;

use anyhow::{Context, Result};

mod models;
mod templates;

use models::to_rust_module;

pub fn generate_api(spec: &str) -> Result<(String, String)> {
    let openapi: OpenAPI = serde_yaml::from_str(spec).expect("Could not deserialize input");

    let rust_module = to_rust_module(&openapi).context("Could not generate rust module")?;

    let serialized_model = serde_yaml::to_string(&rust_module)?;
    let serialized = templates::render_rust_module(templates::RustModule::default())?;

    return Ok((serialized_model, serialized));
}
