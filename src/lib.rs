#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

mod generator;
use anyhow::Result;
pub use generator::OpenapiWithMeta;
mod openapictx;

pub fn generate_api(docs_path: &str, specs: &[OpenapiWithMeta]) -> Result<(String, String)> {
    generator::generate_api(docs_path, specs)
}
