//! Type system that roughly maps to openapi type system

use std::{
    fmt::{Debug, Display},
    rc::Rc,
};

use convert_case::{Case, Casing};
use indexmap::IndexMap;
use openapiv3::{ObjectType, ParameterData, Schema, SchemaKind, Type};
use serde::{Serialize, Serializer};

use anyhow::{bail, Result};

use crate::openapictx::{GenericParameter, OpenApiCtx, ParameterStore, ParametersType, ToSchema};

pub struct DefinitionMaker<'a> {
    pub ctx: &'a OpenApiCtx<'a>,
    pub store: Vec<Rc<Definition>>,
}

impl<'a> DefinitionMaker<'a> {
    pub fn new(ctx: &'a OpenApiCtx<'a>) -> Self {
        DefinitionMaker {
            ctx,
            store: Vec::new(),
        }
    }

    fn add_definition(&mut self, data: Definition) -> Rc<Definition> {
        let data = Rc::new(data);
        self.store.push(data.clone());
        data
    }

    pub fn to_inline(&mut self, name: String, schema: &Schema) -> InlineType {
        let SchemaKind::Type(schema_type) = &schema.schema_kind else {panic!("Only type schemas are implemented")};

        let itype = match schema_type {
            Type::String(_) => InlineType::Scalar(Scalar::String),
            Type::Number(_) => InlineType::Scalar(Scalar::Float),
            Type::Integer(_) => InlineType::Scalar(Scalar::Integer),
            Type::Boolean {} => InlineType::Scalar(Scalar::Boolean),
            Type::Object(val) => {
                let name = get_schema_name(name, &schema.schema_data.title);

                let def_made = self.object_to_definition(name, val);

                let definition = self.add_definition(def_made);

                InlineType::Reference(definition)
            }
            Type::Array(val) => {
                let new_inline = match &val.items {
                    Some(value) => {
                        let deref = self.ctx.deref(value);
                        self.to_inline(format!("{name}Item"), deref)
                    }
                    None => InlineType::Scalar(Scalar::Any),
                };
                InlineType::Array(Box::new(new_inline))
            }
        };
        itype
    }

    /// Returns definition and hashmap of inner schemas to render later
    pub fn object_to_definition(&mut self, name: String, val: &ObjectType) -> Definition {
        let mut properties = IndexMap::new();

        for (prop_name, prop_schema) in val.properties.iter() {
            let prop_schema = self.ctx.deref(prop_schema);
            let prop_name_camel = prop_name.to_case(Case::UpperCamel);
            let itype = self.to_inline(format!("{name}{prop_name_camel}"), prop_schema);
            properties.insert(prop_name.clone(), itype);
        }

        Definition {
            name: name,
            data: DefinitionData::Struct(RStruct {
                properties: properties,
            }),
        }
    }

    fn parameter_data_to_inline(&mut self, param: &ParameterData) -> Result<InlineType> {
        let schema = param.format.to_schema(self.ctx)?;
        let inline = self.to_inline(param.name.to_case(Case::UpperCamel), schema);
        Ok(inline)
    }

    pub fn params_to_inline<T: GenericParameter>(
        &mut self,
        name: String,
        params: &Vec<T>,
    ) -> Result<Option<InlineType>>
    where
        T: GenericParameter,
        Vec<T>: ParameterStore,
    {
        if params.len() == 0 {
            return Ok(None);
        }
        let mut properties = IndexMap::new();
        for param in params {
            let inline = self.parameter_data_to_inline(param.data())?;
            match properties.insert(param.data().name.clone(), inline) {
                Some(_) => bail!("Duplicate parameter name"),
                None => (),
            }
        }

        let def = Rc::new(Definition {
            name: name,
            data: DefinitionData::Struct(RStruct {
                properties: properties,
            }),
        });

        self.store.push(def.clone());

        let inner_type = Box::new(InlineType::Reference(def));

        let result = match Vec::<T>::get_parameters_type() {
            ParametersType::Query => InlineType::Query(inner_type),
            ParametersType::Path => InlineType::Path(inner_type),
            ParametersType::Header => bail!("Header parameters not implemented"),
            ParametersType::Cookie => bail!("Cookie parameters not implemented"),
        };

        Ok(Some(result))
    }
}

#[derive(Debug, Serialize)]
pub enum Scalar {
    String,
    Integer,
    Float,
    Boolean,
    Any,
}

impl Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scalar::String => write!(f, "String"),
            Scalar::Integer => write!(f, "i64"),
            Scalar::Float => write!(f, "f64"),
            Scalar::Boolean => write!(f, "bool"),
            Scalar::Any => write!(f, "Value"),
        }
    }
}

/// Arbitrary inline type
#[derive(Debug)]
pub enum InlineType {
    Scalar(Scalar),
    Array(Box<InlineType>),
    Map(Box<InlineType>),
    Json(Box<InlineType>),  // web::Json
    Path(Box<InlineType>),  // web::Path
    Query(Box<InlineType>), // web::Query
    Reference(Rc<Definition>),
}

impl Display for InlineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InlineType::Scalar(value) => Display::fmt(&value, f),
            InlineType::Array(item) => write!(f, "Vec<{item}>"),
            InlineType::Map(item) => write!(f, "HashMap<String,{item}>"),
            InlineType::Json(item) => write!(f, "web::Json<{item}>"),
            InlineType::Path(item) => write!(f, "web::Path<{item}>"),
            InlineType::Query(item) => write!(f, "web::Query<{item}>"),
            InlineType::Reference(item) => Display::fmt(&item.name, f),
        }
    }
}

impl Serialize for InlineType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct RStruct {
    pub properties: IndexMap<String, InlineType>,
}

#[derive(Debug, Serialize)]
pub struct REnum {
    variants: Vec<String>,
}

#[derive(Debug, Serialize)]
pub enum DefinitionData {
    Struct(RStruct),
    Enum(REnum),
}

#[derive(Debug, Serialize)]
pub struct Definition {
    pub name: String,
    pub data: DefinitionData,
}

fn get_schema_name(name: String, title: &Option<String>) -> String {
    if let Some(val) = title {
        return val.clone();
    };

    return name;
}
