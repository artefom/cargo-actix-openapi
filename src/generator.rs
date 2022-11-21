use openapiv3::OpenAPI;
use serde_yaml;

use anyhow::{Context, Result};

mod models;

use self::models::to_rust_module;

pub fn generate_api(spec: &str) -> Result<String> {
    let openapi: OpenAPI = serde_yaml::from_str(spec).expect("Could not deserialize input");

    let rust_module = to_rust_module(&openapi).context("Could not generate rust module")?;

    let serialized = serde_yaml::to_string(&rust_module)?;

    return Ok(format!("{}", serialized));
}
