use std::{collections::HashMap, rc::Rc};

use openapiv3::OpenAPI;
use serde::ser;

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
                value: variant.value.to_string(),
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

const RUST_KEYWORDS: &[&str] = &["match"];

fn to_rust_identifier(val: &str) -> String {
    let mut val = slug::slugify(val).replace('-', "_");

    if RUST_KEYWORDS.contains(&val.as_str()) {
        val = format!("{val}_");
    }

    val
}

fn render_annotation(vals: HashMap<String, String>) -> Option<String> {
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

        for (prop_name, prop_type) in &struct_def.properties {
            let title = to_rust_identifier(prop_name);

            let mut annotation = HashMap::new();

            if prop_name != &title {
                annotation.insert("rename".to_string(), prop_name.clone());
            };

            props.push(templates::RustProp {
                title,
                annotation: render_annotation(annotation),
                ptype: prop_type.to_string(),
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

pub fn generate_api(spec: &str) -> Result<(String, String)> {
    let openapi: OpenAPI = serde_yaml::from_str(spec).expect("Could not deserialize input");

    let rust_module = to_rust_module(&openapi).context("Could not generate rust module")?;

    let serialized_model = serde_yaml::to_string(&rust_module)?;

    let rust_module = templates::RustModule {
        structs: convert_structs(&rust_module.api.definitions),
        enums: convert_enums(&rust_module.api.definitions),
    };

    let serialized = templates::render_rust_module(rust_module)?;

    Ok((serialized_model, serialized))
}
