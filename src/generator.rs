use std::collections::HashMap;

use openapiv3::{Components, OpenAPI, Parameter};
use serde_yaml;

use anyhow::{bail, Context, Result};

use self::models::{types::Struct, Namespace};

mod models;
mod utils;

use convert_case::{Case, Casing};

fn get_query_params_struct_name(op_id: &String) -> String {
    format!("{}Query", op_id.to_case(Case::UpperCamel))
}

fn get_path_params_struct_name(op_id: &String) -> String {
    format!("{}Path", op_id.to_case(Case::UpperCamel))
}

/// Get operation query parameters
fn get_query_params<'a>(
    global_params: &'a Vec<&openapiv3::Parameter>,
    local_params: &'a Vec<&openapiv3::Parameter>,
) -> Vec<&'a openapiv3::Parameter> {
    let mut result = Vec::new();

    for param in global_params.iter().chain(local_params.iter()) {
        match param {
            Parameter::Query {
                parameter_data: _,
                allow_reserved: _,
                style: _,
                allow_empty_value: _,
            } => result.push(*param),
            _ => continue,
        }
    }

    result
}

/// Get operation path parameters
fn get_path_params<'a>(
    global_params: &'a Vec<&openapiv3::Parameter>,
    local_params: &'a Vec<&openapiv3::Parameter>,
) -> Vec<&'a openapiv3::Parameter> {
    let mut result = Vec::new();

    for param in global_params.iter().chain(local_params.iter()) {
        match param {
            Parameter::Path {
                parameter_data: _,
                style: _,
            } => result.push(*param),
            _ => continue,
        }
    }

    result
}

/// Convert parameters into rust struct and return reference
fn to_rust_struct(title: &str, params: &Vec<&openapiv3::Parameter>) -> Option<Struct> {
    if params.len() == 0 {
        return None;
    };

    let fields = Vec::new();

    Some(Struct {
        name: title.to_string(),
        fields: fields,
    })
}

/// Convert parameters into rust struct and return reference
fn to_rust_structref(
    namespace: &mut models::Namespace,
    title: &str,
    params: &Vec<&openapiv3::Parameter>,
) -> Option<StructRef> {
    to_rust_struct(title, params).and_then(|x| Some(namespace.add_struct(x)))
}

fn to_rust_operation(
    namespace: &mut models::Namespace,
    components: &Option<Components>,
    path: &str,
    method: models::HttpMethod,
    operation: &openapiv3::Operation,
    global_params: &Vec<&openapiv3::Parameter>,
) -> Result<models::Operation> {
    // Get operation name
    let Some(name) = &operation.operation_id else {
        bail!("Operation must have operation_id")
    };

    // Get operation docstring
    let doc = operation
        .summary
        .as_ref()
        .or(operation.description.as_ref())
        .cloned();

    // Dereferenced method parameters
    let method_params: Vec<&Parameter> = operation
        .parameters
        .iter()
        .map(|x| utils::deref(components, x))
        .collect();

    let path_parameters = get_path_params(global_params, &method_params);
    let query_parameters = get_query_params(global_params, &method_params);

    // Create and register path params struct
    let path_params = to_rust_structref(
        namespace,
        &get_path_params_struct_name(name),
        &path_parameters,
    );

    // Create and register query params struct
    let query_params = to_rust_structref(
        namespace,
        &&get_query_params_struct_name(name),
        &query_parameters,
    );

    Ok(models::Operation {
        name: name.clone(),
        path: path.to_string(),
        method: method,

        doc: doc,
        param_path: path_params,
        param_query: query_params,
        param_body: None,

        // Response
        // -----------------------------
        response: None,
        error: None,
    })
}

fn to_operation_map(
    path_item: &openapiv3::PathItem,
) -> HashMap<models::HttpMethod, &openapiv3::Operation> {
    let mut result = HashMap::new();

    if let Some(op) = &path_item.get {
        result.insert(models::HttpMethod::Get, op);
    }

    if let Some(op) = &path_item.post {
        result.insert(models::HttpMethod::Post, op);
    }

    if let Some(op) = &path_item.delete {
        result.insert(models::HttpMethod::Delete, op);
    }

    result
}

pub fn to_rust_module(spec: &OpenAPI) -> Result<models::RustModule> {
    let mut operations = Vec::new();

    let mut namespace = Namespace::new();

    for (path, path_item) in spec.paths.iter() {
        let path_item = utils::deref(&spec.components, path_item);
        let global_params: Vec<&openapiv3::Parameter> = path_item
            .parameters
            .iter()
            .map(|x| utils::deref(&spec.components, x))
            .collect();
        for (method, operation) in to_operation_map(path_item) {
            operations.push(
                to_rust_operation(
                    &mut namespace,
                    &spec.components,
                    path,
                    method,
                    operation,
                    &global_params,
                )
                .with_context(|| {
                    format!(
                        "Could not convert to rust operation at {} {}",
                        &method, &path
                    )
                })?,
            );
        }
    }

    Ok(models::RustModule {
        namespace: namespace,
        api: models::ApiService {
            operations: operations,
        },
    })
}

pub fn generate_api(spec: &str) -> Result<String> {
    let openapi: OpenAPI = serde_yaml::from_str(spec).expect("Could not deserialize input");

    let rust_module = to_rust_module(&openapi).context("Could not generate rust module")?;

    let serialized = serde_yaml::to_string(&rust_module)?;

    return Ok(format!("{}", serialized));
}
