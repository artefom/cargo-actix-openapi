//! Type system that roughly maps to openapi type system

use std::{
    fmt::{Debug, Display},
    rc::Rc,
};

use convert_case::{Case, Casing};
use indexmap::IndexMap;
use openapiv3::{ObjectType, ParameterData, ReferenceOr, RequestBody, Schema, SchemaKind, Type};
use serde::{Serialize, Serializer};

use anyhow::{bail, Result};

use crate::openapictx::{
    CookieParameter, HeaderParaemter, OpenApiCtx, ParameterStore, ParametersType, PathParameter,
    QueryParameter, ToSchema,
};

pub trait GenericParameter {
    fn data(&self) -> &ParameterData;
}

impl<'a> GenericParameter for QueryParameter<'a> {
    fn data(&self) -> &ParameterData {
        self.parameter_data
    }
}

impl<'a> GenericParameter for HeaderParaemter<'a> {
    fn data(&self) -> &ParameterData {
        self.parameter_data
    }
}

impl<'a> GenericParameter for PathParameter<'a> {
    fn data(&self) -> &ParameterData {
        self.parameter_data
    }
}

impl<'a> GenericParameter for CookieParameter<'a> {
    fn data(&self) -> &ParameterData {
        self.parameter_data
    }
}

pub trait MaybeInlining {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<Option<InlineType>>;
}

impl<T> MaybeInlining for Option<T>
where
    T: Inlining,
{
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<Option<InlineType>> {
        match self {
            Some(value) => Ok(Some(value.inline(name, defmaker)?)),
            None => Ok(None),
        }
    }
}

impl<T> MaybeInlining for ReferenceOr<T>
where
    T: MaybeInlining,
{
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<Option<InlineType>> {
        let deref = defmaker.ctx.deref(self);
        deref.inline(name, defmaker)
    }
}

pub trait Inlining {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType>;
}

impl<T> Inlining for ReferenceOr<T>
where
    T: Inlining,
{
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let deref = defmaker.ctx.deref(self);
        deref.inline(name, defmaker)
    }
}

impl Inlining for RequestBody {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let schema = self.content.to_schema(defmaker.ctx)?;
        let inner = InlineType::Json(Box::new(schema.inline(name, defmaker)?));
        if self.required {
            Ok(inner)
        } else {
            Ok(InlineType::Option(Box::new(inner)))
        }
    }
}

impl Inlining for ParameterData {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let schema = self.format.to_schema(defmaker.ctx)?;
        let inner = schema.inline(name, defmaker)?;
        if self.required {
            Ok(inner)
        } else {
            Ok(InlineType::Option(Box::new(inner)))
        }
    }
}

impl<T> MaybeInlining for Vec<T>
where
    T: GenericParameter,
    Vec<T>: ParameterStore,
{
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<Option<InlineType>> {
        if self.len() == 0 {
            return Ok(None);
        }
        let mut properties = IndexMap::new();
        for param in self {
            let param_data = param.data();
            let inline_name = format!("{}{}", &name, param_data.name.to_case(Case::UpperCamel));
            let inline = param.data().inline(inline_name, defmaker)?;
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

        defmaker.store.push(def.clone());

        let inner_type = Box::new(InlineType::Reference(def));

        Ok(Some(match Vec::<T>::get_parameters_type() {
            ParametersType::Query => InlineType::Query(inner_type),
            ParametersType::Path => InlineType::Path(inner_type),
            ParametersType::Header => bail!("Header parameters not implemented"),
            ParametersType::Cookie => bail!("Cookie parameters not implemented"),
        }))
    }
}

impl Inlining for Schema {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let SchemaKind::Type(schema_type) = &self.schema_kind else {panic!("Only type schemas are implemented")};

        let itype = match schema_type {
            Type::String(_) => InlineType::String,
            Type::Number(_) => InlineType::Float,
            Type::Integer(_) => InlineType::Integer,
            Type::Boolean {} => InlineType::Boolean,
            Type::Object(val) => {
                let name = get_schema_name(name, &self.schema_data.title);
                val.inline(name, defmaker)?
            }
            Type::Array(val) => {
                let new_inline = match &val.items {
                    Some(value) => {
                        let deref = defmaker.ctx.deref(value);
                        deref.inline(format!("{name}Item"), defmaker)?
                    }
                    None => InlineType::Any,
                };
                InlineType::Array(Box::new(new_inline))
            }
        };
        Ok(itype)
    }
}

impl Inlining for ObjectType {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let mut properties = IndexMap::new();

        for (prop_name, prop_schema) in self.properties.iter() {
            let prop_schema = defmaker.ctx.deref(prop_schema);
            let prop_name_camel = prop_name.to_case(Case::UpperCamel);
            let itype = prop_schema.inline(format!("{name}{prop_name_camel}"), defmaker)?;
            properties.insert(prop_name.clone(), itype);
        }

        let definition = Rc::new(Definition {
            name: name,
            data: DefinitionData::Struct(RStruct {
                properties: properties,
            }),
        });

        defmaker.store.push(definition.clone());

        Ok(InlineType::Reference(definition))
    }
}

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
}

/// Arbitrary inline type
#[derive(Debug)]
pub enum InlineType {
    String,
    Integer,
    Float,
    Boolean,
    Any,
    Array(Box<InlineType>),  // Vec::<InlineType>
    Map(Box<InlineType>),    // HashMap::<String, InlineType>
    Json(Box<InlineType>),   // web::Json
    Path(Box<InlineType>),   // web::Path
    Query(Box<InlineType>),  // web::Query
    Option(Box<InlineType>), // Option<InlineType>
    Reference(Rc<Definition>),
}

impl Display for InlineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InlineType::String => write!(f, "String"),
            InlineType::Integer => write!(f, "i64"),
            InlineType::Float => write!(f, "f64"),
            InlineType::Boolean => write!(f, "bool"),
            InlineType::Any => write!(f, "serde_json::Value"),
            InlineType::Array(item) => write!(f, "Vec<{item}>"),
            InlineType::Map(item) => write!(f, "HashMap<String,{item}>"),
            InlineType::Json(item) => write!(f, "web::Json<{item}>"),
            InlineType::Path(item) => write!(f, "web::Path<{item}>"),
            InlineType::Query(item) => write!(f, "web::Query<{item}>"),
            InlineType::Option(item) => write!(f, "Option<{item}>"),
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

/// Something that can serialize into rust struct property
struct RProp {}

/// Something that can serialize into rust struct
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
