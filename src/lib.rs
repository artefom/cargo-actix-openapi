#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

mod generator;
use anyhow::Result;

pub fn generate_api(spec: &str) -> Result<String> {
    generator::generate_api(spec)
}
