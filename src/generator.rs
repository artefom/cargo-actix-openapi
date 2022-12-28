use std::rc::Rc;

use openapiv3::OpenAPI;
use serde::ser;
use serde_yaml;

use anyhow::{Context, Result};

mod models;
mod templates;

use models::to_rust_module;

fn convert_enums(defs: &Vec<Rc<models::types::Definition>>) -> Vec<templates::RustEnum> {
    let mut enums = Vec::new();

    for definition in defs {
        let enum_def = match &definition.data {
            models::types::DefinitionData::Enum(value) => value,
            _ => continue,
        };

        let mut variants = Vec::new();

        for variant in &enum_def.variants {
            variants.push(templates::RustEnumVariant {
                title: variant.name.clone(),
                value: format!("{}", variant.value),
            })
        }

        enums.push(templates::RustEnum {
            title: definition.name.clone(),
            variants: variants,
        })
    }

    enums
}

pub fn generate_api(spec: &str) -> Result<(String, String)> {
    let openapi: OpenAPI = serde_yaml::from_str(spec).expect("Could not deserialize input");

    let rust_module = to_rust_module(&openapi).context("Could not generate rust module")?;

    let serialized_model = serde_yaml::to_string(&rust_module)?;

    let rust_module = templates::RustModule {
        enums: convert_enums(&rust_module.api.definitions),
    };

    let serialized = templates::render_rust_module(rust_module)?;

    return Ok((serialized_model, serialized));
}
