use serde::{Deserialize, Serialize};
use std::fmt::Display;
pub mod types;

/// Reference to ApiErr definition
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrRef(pub String);

/// Http method
#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Serialize, Deserialize)]
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

/// Any definition
#[derive(Debug, Serialize, Deserialize)]
pub enum Definition {
    Struct(types::Struct),
    Enum(types::Enum),
    ApiError(types::ApiError),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Namespace {
    pub definitions: Vec<Definition>,
}

impl Namespace {
    pub fn new() -> Self {
        Namespace {
            definitions: Vec::new(),
        }
    }
    pub fn add_struct(&mut self, data: types::Struct) -> types::StructRef {
        let ref_name = data.name.clone();
        self.definitions.push(Definition::Struct(data));
        types::StructRef(ref_name)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
    pub name: String,
    pub path: String,
    pub method: HttpMethod, // Operation method

    pub doc: Option<String>,
    pub param_path: Option<StructRef>,  // web::Path
    pub param_query: Option<StructRef>, // web::Query
    pub param_body: Option<StructRef>,  // web::Json

    // Response
    // -----------------------------
    pub response: Option<types::Any>,
    pub error: Option<ApiErrRef>, // Error type
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiService {
    pub operations: Vec<Operation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RustModule {
    pub namespace: Namespace,
    pub api: ApiService,
}
