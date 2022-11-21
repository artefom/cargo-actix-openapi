use convert_case::{Case, Casing};
use openapiv3::{OpenAPI, Parameter, ReferenceOr};
use serde::{ser::SerializeTuple, Serialize, Serializer};
use std::{collections::HashMap, fmt::Display, ops::Deref, rc::Rc};
pub mod types;
use anyhow::{bail, Context, Result};

use crate::openapictx::{OpenApiCtx, ToSchema};

use self::types::{Definition, DefinitionMaker, InlineType};

/// Reference to ApiErr definition
#[derive(Debug, Serialize)]
pub struct ApiErrRef(pub String);

/// Http method
#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Serialize)]
pub enum HttpMethod {
    Post,
    Get,
    Delete,
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Delete => write!(f, "DELETE"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Operation {
    pub name: String,
    pub path: String,
    pub method: HttpMethod, // Operation method

    pub doc: Option<String>,
    pub param_path: Option<InlineType>,  // web::Path
    pub param_query: Option<InlineType>, // web::Query
    pub param_body: Option<InlineType>,  // web::Json

    // Response
    // -----------------------------
    pub response: Option<String>,
    pub error: Option<ApiErrRef>, // Error type
}

fn serialize_def_vec<S>(data: &Vec<Rc<Definition>>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut seq = s.serialize_tuple(data.len())?;
    for val in data {
        seq.serialize_element(val.deref())?;
    }
    seq.end()
}

#[derive(Debug, Serialize)]
pub struct ApiService {
    #[serde(serialize_with = "serialize_def_vec")]
    pub definitions: Vec<Rc<Definition>>,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Serialize)]
pub struct RustModule {
    pub api: ApiService,
}

fn to_operation_map(path_item: &openapiv3::PathItem) -> HashMap<HttpMethod, &openapiv3::Operation> {
    let mut result = HashMap::new();

    if let Some(op) = &path_item.get {
        result.insert(HttpMethod::Get, op);
    }

    if let Some(op) = &path_item.post {
        result.insert(HttpMethod::Post, op);
    }

    if let Some(op) = &path_item.delete {
        result.insert(HttpMethod::Delete, op);
    }

    result
}

fn to_rust_operation(
    ctx: &OpenApiCtx,
    defmaker: &mut DefinitionMaker,
    path: &str,
    method: HttpMethod,
    operation: &openapiv3::Operation,
    global_params: &Vec<ReferenceOr<Parameter>>,
) -> Result<Operation> {
    // Get operation name
    let Some(name) = &operation.operation_id else {
        bail!("Operation must have operation_id")
    };

    let name_upper = name.to_case(Case::UpperCamel);

    // Get operation docstring
    let doc = operation
        .summary
        .as_ref()
        .or(operation.description.as_ref())
        .cloned();

    let params_spliited = ctx.split_parameters(global_params, &operation.parameters);

    let path_params_inline = defmaker.params_to_inline(
        format!("{name_upper}Path"),
        &params_spliited.path_parameters,
    )?;

    let query_params_inline = defmaker.params_to_inline(
        format!("{name_upper}Query"),
        &params_spliited.query_parameters,
    )?;

    if params_spliited.header_parameters.len() != 0 {
        bail!("Header parameters not supported")
    };

    if params_spliited.cookie_parameters.len() != 0 {
        bail!("Cookie parameters not supported")
    };

    let param_body = match &operation.request_body {
        Some(value) => {
            let body = ctx.deref(value);
            let inline = defmaker.to_inline(format!("{name_upper}Body"), body.to_schema(ctx)?);
            Some(InlineType::Json(Box::new(inline)))
        }
        None => None,
    };

    Ok(Operation {
        name: name.clone(),
        path: path.to_string(),
        method: method,

        doc: doc,
        param_path: path_params_inline,
        param_query: query_params_inline,
        param_body: param_body,

        // Response
        // -----------------------------
        response: None,
        error: None,
    })
}

pub fn to_rust_module(spec: &OpenAPI) -> Result<RustModule> {
    let mut operations = Vec::new();

    let ctx = OpenApiCtx::new(&spec.components);

    let mut defmaker = DefinitionMaker::new(&ctx);

    for (path, path_item) in spec.paths.iter() {
        let path_item = ctx.deref(path_item);
        let global_params: &Vec<ReferenceOr<Parameter>> = &path_item.parameters;
        for (method, operation) in to_operation_map(path_item) {
            operations.push(
                to_rust_operation(&ctx, &mut defmaker, path, method, operation, global_params)
                    .with_context(|| {
                        format!(
                            "Could not convert to rust operation at {} {}",
                            &method, &path
                        )
                    })?,
            );
        }
    }

    Ok(RustModule {
        api: ApiService {
            definitions: defmaker.store,
            operations: operations,
        },
    })
}
