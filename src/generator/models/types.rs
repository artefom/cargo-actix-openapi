//! Type system that roughly maps to openapi type system

use std::fmt::Debug;

use serde::{Deserialize, Serialize};

pub trait Referenceable: Debug {
    fn name(&self) -> &String;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Any {
    Scalar(Scalar),
    Array(Array),
    Map(Map),
    Reference(String), // Reference to something by name (another struct or enum)
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Scalar {
    String,
    Integer,
    Float,
    Boolean,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StructProperty {
    pub name: String,
    pub fomrat: Any,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<StructProperty>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Enum {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub name: String,
}

/// Vec<String, T>
#[derive(Debug, Serialize, Deserialize)]
pub struct Array(Box<Any>);

/// HashMap<String, T>
#[derive(Debug, Serialize, Deserialize)]
pub struct Map(Box<Any>);
