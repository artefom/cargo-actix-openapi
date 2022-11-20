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
