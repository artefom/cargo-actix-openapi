use convert_case::Case;
use indexmap::IndexMap;
use openapiv3::{OpenAPI, Parameter, ReferenceOr};
use serde::{ser::SerializeTuple, Serialize, Serializer};
use std::{fmt::Display, ops::Deref, rc::Rc};
pub mod types;
use anyhow::{bail, Context, Result};

use crate::openapictx::OpenApiCtx;

use self::types::{
    to_rust_identifier, Definition, DefinitionMaker, InlineType, Inlining, MaybeInlining,
};

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
            HttpMethod::Post => write!(f, "post"),
            HttpMethod::Get => write!(f, "get"),
            HttpMethod::Delete => write!(f, "delete"),
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
    pub response: InlineType,
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

fn to_operation_map(
    path_item: &openapiv3::PathItem,
) -> IndexMap<HttpMethod, &openapiv3::Operation> {
    let mut result = IndexMap::new();

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
    global_params: &[ReferenceOr<Parameter>],
) -> Result<Operation> {
    // Get operation name
    let Some(name) = &operation.operation_id else {
        bail!("Operation must have operation_id")
    };

    let name_upper = to_rust_identifier(name, Case::UpperCamel);

    // Get operation docstring
    let doc = operation
        .summary
        .as_ref()
        .or(operation.description.as_ref())
        .cloned();

    let params_spliited = ctx.split_parameters(global_params, &operation.parameters)?;

    let path_params_inline = params_spliited
        .path_parameters
        .inline(format!("{name_upper}Path"), defmaker)?;

    let query_params_inline = params_spliited
        .query_parameters
        .inline(format!("{name_upper}Query"), defmaker)?;

    if !params_spliited.header_parameters.is_empty() {
        bail!("Header parameters not supported")
    };

    if !params_spliited.cookie_parameters.is_empty() {
        bail!("Cookie parameters not supported")
    };

    let param_body = operation
        .request_body
        .inline(format!("{name_upper}Body"), defmaker)?;

    let response = operation
        .responses
        .inline(format!("{name_upper}Response"), defmaker)?;

    Ok(Operation {
        name: name.clone(),
        path: path.to_string(),
        method,

        doc,
        param_path: path_params_inline,
        param_query: query_params_inline,
        param_body,

        // Response
        // -----------------------------
        response,
    })
}

pub fn to_rust_module(spec: &OpenAPI) -> Result<RustModule> {
    let mut operations = Vec::new();

    let ctx = OpenApiCtx::new(&spec.components);

    let mut defmaker = DefinitionMaker::new(&ctx);

    for (path, path_item) in spec.paths.iter() {
        let path_item = ctx.deref(path_item)?;
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
            definitions: defmaker.dedup_store,
            operations,
        },
    })
}
