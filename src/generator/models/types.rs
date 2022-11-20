//! Type system that roughly maps to openapi type system

use std::{collections::HashMap, fmt::Debug, rc::Rc};

use openapiv3::{Components, ObjectType, ReferenceOr, Schema, SchemaKind, Type};
use serde::{Deserialize, Serialize};

use anyhow::{bail, Result};

struct DefinitionStore<'a> {
    store: Vec<Definition<'a>>,
}

impl<'a> DefinitionStore<'a> {
    fn add_definition(&'a mut self, data: Definition<'a>) -> &'a Definition<'a> {
        self.store.push(data);
        self.store.last().unwrap()
    }
}

pub enum Scalar {
    String,
    Integer,
    Float,
    Boolean,
}

/// Arbitrary inline type
pub enum InlineType<'a> {
    Scalar(Scalar),
    Array(Box<InlineType<'a>>),
    Map(Box<InlineType<'a>>),
    Reference(&'a Definition<'a>),
}

pub struct RStruct<'a> {
    pub properties: HashMap<String, InlineType<'a>>,
}

pub struct REnum {
    variants: Vec<String>,
}

enum DefinitionData<'a> {
    Struct(RStruct<'a>),
    Enum(REnum),
}

struct Definition<'a> {
    name: String,
    data: DefinitionData<'a>,
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

fn make_definition<'a>(
    components: &'a Option<Components>,
    definable: Definable<'a>,
) -> Result<(Definition<'a>, Vec<Definition<'a>>)> {
    match definable.data {
        DefinableData::Object(obj) => {
            let (definition, inner) = get_definition(components, definable.name, obj)?;
            Ok((definition, inner))
        }
    }
}

fn to_inline<'a>(
    components: &'a Option<Components>,
    schema: &'a ReferenceOr<Box<Schema>>,
) -> Result<(InlineType<'a>, Vec<Definition<'a>>)> {
    let schema = deref(components, schema);

    let SchemaKind::Type(schema_type) = &schema.schema_kind else {panic!("Nice")};
    let mut definitions = Vec::new();

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

            let (definition, more_defs) = make_definition(components, definable)?;

            InlineType::Reference(&definition)
        }
        Type::Array(val) => {
            let Some(items) = &val.items else {
                bail!("Array items must not be null")
            };
            let (new_inline, more_defs) = to_inline(components, items)?;
            definitions.extend(more_defs.into_iter());
            InlineType::Array(Box::new(new_inline))
        }
    };
    Ok((itype, definitions))
}

/// Returns definition and hashmap of inner schemas to render later
fn get_definition<'a>(
    components: &'a Option<Components>,
    name: String,
    val: &'a ObjectType,
) -> Result<(Definition<'a>, Vec<Definition<'a>>)> {
    let mut inner = Vec::new();
    let mut properties = HashMap::new();

    for (prop_name, prop_schema) in val.properties.iter() {
        let (itype, definitions) = to_inline(components, prop_schema)?;
        inner.extend(definitions.into_iter());

        properties.insert(prop_name.clone(), itype);
    }

    let definition = Definition {
        name: name,
        data: DefinitionData::Struct(RStruct {
            properties: properties,
        }),
    };

    Ok((definition, inner))
}

// trait NestedType {
//     type Flat;

//     /// Convert this nested type to flat
//     /// And return all references as hashmap
//     fn to_flat(
//         &self,
//     ) -> (
//         Self::Flat,
//         HashMap<String, Box<dyn NestedType<Flat = Self::Flat>>>,
//     );
// }

// /// Unwraps to flat all nested types
// pub fn to_flat_all<N, F>(nested: N) -> Result<(F, HashMap<String, F>)>
// where
//     N: NestedType<Flat = F>,
// {
//     let result: HashMap<String, F> = HashMap::new();

//     let mut nested_stack = Vec::new();

//     let (first_flat, nested) = nested.to_flat();

//     nested_stack.extend(nested.into_iter());

//     loop {
//         let Some((next_name, next)) = nested_stack.pop() else {break};
//         let (next_flat, next_nested) = next.to_flat();

//         match result.insert(next_name, next_flat) {
//             Some(_) => bail!("Duplicate nested name"),
//             None => (),
//         };

//         nested_stack.extend(next_nested.into_iter());
//     }

//     Ok((first_flat, result))
// }

// enum Scalar {
//     String,
//     Integer,
//     Float,
//     Boolean,
// }

// struct RStruct {
//     properties: HashMap<String, AnyNested>
// }

// enum AnyNested {
//     Scalar(Scalar),
//     Array(Box<AnyNested>),
//     Map(Box<AnyNested>),
// }

// /// Flat type
// enum AnyFlat {
//     Scalar(Scalar),
//     Array(Box<AnyFlat>),
//     Map(Box<AnyFlat>),
//     Struct(String), // Reference to some struct
//     Enum(String),   // Reference to some enum
// }

// Flattener of various types
// N - nested type (Does contain nested types)
// F - flat type (Does not contain nested types)
// pub struct TypeStore<F> {
//     definitions: HashMap<String, F>, // All named flat definitions
// }

// impl<F> TypeStore<F> {
//     /// Get all definitions
//     fn definitions(&self) -> HashMap<&String, &F> {
//         todo!()
//     }

//     /// Add nested type to typestore and get back flat type
//     fn add<N>(&mut self, data: N) -> F {
//         todo!()
//     }
// }

// enum ScalarOrReference<T> {
//     String,
//     Integer,
//     Float,
//     Boolean,
//     Reference(String, T), // Name and type to which this refers
// }

// trait Flattable<T> {
//     /// Converts this object to scalar type or reference to something
//     fn to_scalar_or_reference(&self) -> ScalarOrReference<T>;
// }

// pub trait FlatType {}

// pub enum InnerNestedTypes<T: NestedType> {
//     SingleType(T),
//     TypeMap(HashMap<String, T>),
// }

// pub trait NestedType {
//     type Nested: NestedType;
//     type Flat: FlatType;

//     fn inner_types() -> InnerNestedTypes<Self::Nested>;
// }

// pub enum AnyNested {
//     String,
//     Integer,
//     Float,
//     Boolean,
//     Array(RArray),
// }

// struct RArray {
//     items: Box<AnyNested>
// }

// pub trait Referenceable: Debug {
//     fn name(&self) -> &String;
// }

// #[derive(Debug, Serialize, Deserialize)]
// pub enum Any {
//     Scalar(Scalar),
//     Array(Array),
//     Map(Map),
//     Reference(String), // Reference to something by name (another struct or enum)
// }

// #[derive(Debug, Serialize, Deserialize)]
// pub enum Scalar {
//     String,
//     Integer,
//     Float,
//     Boolean,
// }

// #[derive(Debug, Serialize, Deserialize)]
// pub struct StructProperty {
//     pub name: String,
//     pub fomrat: Any,
// }

// #[derive(Debug, Serialize, Deserialize)]
// pub struct Struct {
//     pub name: String,
//     pub fields: Vec<StructProperty>,
// }

// #[derive(Debug, Serialize, Deserialize)]
// pub struct Enum {
//     pub name: String,
// }

// #[derive(Debug, Serialize, Deserialize)]
// pub struct ApiError {
//     pub name: String,
// }

// /// Vec<String, T>
// #[derive(Debug, Serialize, Deserialize)]
// pub struct Array(Box<Any>);

// /// HashMap<String, T>
// #[derive(Debug, Serialize, Deserialize)]
// pub struct Map(Box<Any>);
