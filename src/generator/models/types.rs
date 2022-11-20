//! Type system that roughly maps to openapi type system

use std::{collections::HashMap, fmt::Debug, rc::Rc};

use openapiv3::{Components, ObjectType, ReferenceOr, Schema, SchemaKind, Type};
use serde::{Deserialize, Serialize};

use anyhow::{bail, Result};

struct DefinitionStore {
    store: Vec<Rc<Definition>>,
}

impl DefinitionStore {
    fn add_definition(&mut self, data: Definition) -> Rc<Definition> {
        let data = Rc::new(data);
        self.store.push(data.clone());
        data
    }
}

pub enum Scalar {
    String,
    Integer,
    Float,
    Boolean,
}

/// Arbitrary inline type
pub enum InlineType {
    Scalar(Scalar),
    Array(Box<InlineType>),
    Map(Box<InlineType>),
    Reference(Rc<Definition>),
}

pub struct RStruct {
    pub properties: HashMap<String, InlineType>,
}

pub struct REnum {
    variants: Vec<String>,
}

pub enum DefinitionData {
    Struct(RStruct),
    Enum(REnum),
}

pub struct Definition {
    pub name: String,
    pub data: DefinitionData,
}

fn get_schema_name(title: &Option<String>) -> Result<&String> {
    if let Some(val) = title {
        return Ok(val);
    };

    bail!("Title must not be null")
}

/// Dereference openapi object
pub fn deref<'a, T>(_components: &Option<Components>, obj: &'a ReferenceOr<T>) -> &'a T {
    let _obj_ref = match obj {
        ReferenceOr::Reference { reference } => reference,
        ReferenceOr::Item(value) => return value,
    };
    todo!()
}

pub enum DefinableData<'a> {
    Object(&'a ObjectType),
}

pub struct Definable<'a> {
    name: String,
    data: DefinableData<'a>,
}

fn make_definition(
    components: &Option<Components>,
    definable: Definable,
    store: &mut DefinitionStore,
) -> Result<Definition> {
    match definable.data {
        DefinableData::Object(obj) => {
            let definition = get_definition(components, definable.name, obj, store)?;
            Ok(definition)
        }
    }
}

fn to_inline<'a, 'b>(
    components: &Option<Components>,
    schema: &ReferenceOr<Box<Schema>>,
    store: &mut DefinitionStore,
) -> Result<InlineType> {
    let schema = deref(components, schema);

    let SchemaKind::Type(schema_type) = &schema.schema_kind else {panic!("Nice")};

    let itype = match schema_type {
        Type::String(val) => InlineType::Scalar(Scalar::String),
        Type::Number(val) => InlineType::Scalar(Scalar::Float),
        Type::Integer(val) => InlineType::Scalar(Scalar::Integer),
        Type::Boolean {} => InlineType::Scalar(Scalar::Boolean),
        Type::Object(val) => {
            let name = get_schema_name(&schema.schema_data.title)?.clone();
            let definable = Definable {
                name: name.clone(),
                data: DefinableData::Object(val),
            };

            let def_made = make_definition(components, definable, store)?;

            let definition = store.add_definition(def_made);

            InlineType::Reference(definition)
        }
        Type::Array(val) => {
            let Some(items) = &val.items else {
                bail!("Array items must not be null")
            };
            let new_inline = to_inline(components, items, store)?;
            InlineType::Array(Box::new(new_inline))
        }
    };
    Ok(itype)
}

/// Returns definition and hashmap of inner schemas to render later
fn get_definition(
    components: &Option<Components>,
    name: String,
    val: &ObjectType,
    store: &mut DefinitionStore,
) -> Result<Definition> {
    let mut properties = HashMap::new();

    for (prop_name, prop_schema) in val.properties.iter() {
        let itype = to_inline(components, prop_schema, store)?;

        properties.insert(prop_name.clone(), itype);
    }

    let definition = Definition {
        name: name,
        data: DefinitionData::Struct(RStruct {
            properties: properties,
        }),
    };

    Ok(definition)
}
