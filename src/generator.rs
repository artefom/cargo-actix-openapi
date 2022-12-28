use std::{collections::HashMap, hash::Hash, rc::Rc};

use indexmap::IndexMap;
use openapiv3::OpenAPI;
use serde::ser;

use anyhow::{Context, Result};

mod models;
mod templates;

use models::to_rust_module;

use self::models::types::to_rust_identifier;

fn convert_enums(defs: &Vec<Rc<models::types::Definition>>) -> Vec<templates::RustEnum> {
    let mut enums = Vec::new();

    for definition in defs {
        let enum_def = match &definition.data {
            models::types::DefinitionData::Enum(value) => value,
            _ => continue,
        };

        let mut variants = Vec::new();

        for variant in &enum_def.variants {
            let mut annotation = IndexMap::new();

            if variant.value != variant.name {
                annotation.insert("rename", variant.value.clone());
            }

            variants.push(templates::RustEnumVariant {
                title: variant.name.clone(),
                annotation: render_annotation(annotation),
            })
        }

        enums.push(templates::RustEnum {
            doc: enum_def.doc.clone(),
            title: definition.name.clone(),
            variants,
        })
    }

    enums
}

fn render_annotation(vals: IndexMap<&str, String>) -> Option<String> {
    let mut keyvals: Vec<String> = Vec::new();

    for (key, value) in vals {
        let value = templates::quote_str(&value);
        keyvals.push(format!("{key}={value}"))
    }

    if keyvals.is_empty() {
        return None;
    }

    let keyvals = keyvals.join(", ");

    Some(format!("#[serde({keyvals})]"))
}

fn convert_structs(defs: &Vec<Rc<models::types::Definition>>) -> Vec<templates::RustStruct> {
    let mut enums = Vec::new();

    for definition in defs {
        let struct_def = match &definition.data {
            models::types::DefinitionData::Struct(value) => value,
            _ => continue,
        };

        let mut props = Vec::new();

        for prop in &struct_def.properties {
            let mut annotation = IndexMap::new();

            if prop.rename != prop.name {
                annotation.insert("rename", prop.rename.clone());
            };

            if let Some(ref default) = prop.default {
                annotation.insert("default", default.to_string());
            };

            props.push(templates::RustProp {
                title: prop.name.clone(),
                annotation: render_annotation(annotation),
                type_: prop.ptype.to_string(),
            })
        }

        enums.push(templates::RustStruct {
            doc: struct_def.doc.clone(),
            title: definition.name.clone(),
            props,
        })
    }

    enums
}

fn convert_defaults(defs: &Vec<Rc<models::types::Definition>>) -> Vec<templates::RustDefault> {
    let mut defaults = Vec::new();

    for definition in defs {
        let default = match &definition.data {
            models::types::DefinitionData::DefaultProvider(value) => value,
            _ => continue,
        };

        defaults.push(templates::RustDefault {
            title: definition.name.clone(),
            type_: default.vtype.to_string(),
            value: default.value.to_string(),
        })
    }

    defaults
}

pub fn generate_api(spec: &str) -> Result<(String, String)> {
    let openapi: OpenAPI = serde_yaml::from_str(spec).expect("Could not deserialize input");

    let rust_module = to_rust_module(&openapi).context("Could not generate rust module")?;

    let serialized_model = serde_yaml::to_string(&rust_module)?;

    let rust_module = templates::RustModule {
        structs: convert_structs(&rust_module.api.definitions),
        enums: convert_enums(&rust_module.api.definitions),
        defaults: convert_defaults(&rust_module.api.definitions),
    };

    let serialized = templates::render_rust_module(rust_module)?;

    Ok((serialized_model, serialized))
}
