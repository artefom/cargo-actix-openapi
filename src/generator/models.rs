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





#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
    pub name: String,
    pub path: String,
    pub method: HttpMethod, // Operation method

    pub doc: Option<String>,
    pub param_path: Option<String>,  // web::Path
    pub param_query: Option<String>, // web::Query
    pub param_body: Option<String>,  // web::Json

    // Response
    // -----------------------------
    pub response: Option<String>,
    pub error: Option<ApiErrRef>, // Error type
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiService {
    pub operations: Vec<Operation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RustModule {
    pub api: ApiService,
}
