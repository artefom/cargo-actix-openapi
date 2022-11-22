//! Type system that roughly maps to openapi type system

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    ops::Deref,
    rc::Rc,
};

use convert_case::{Case, Casing};
use indexmap::IndexMap;
use openapiv3::{
    MediaType, ObjectType, ParameterData, ReferenceOr, RequestBody, Response, Responses, Schema,
    SchemaKind, StatusCode, Type,
};
use serde::{Serialize, Serializer};

use anyhow::{bail, Result};

use crate::openapictx::{
    CookieParameter, Dereferencing, HeaderParaemter, OpenApiCtx, ParameterStore, ParametersType,
    PathParameter, QueryParameter, ToSchema,
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
    T: MaybeInlining + Dereferencing<T>,
{
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<Option<InlineType>> {
        let deref = defmaker.ctx.deref(self)?;
        deref.inline(name, defmaker)
    }
}

pub trait Inlining {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType>;
}

impl<T> Inlining for ReferenceOr<T>
where
    T: Inlining + Dereferencing<T>,
{
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let deref = defmaker.ctx.deref(self)?;
        deref.inline(name, defmaker)
    }
}

impl Inlining for IndexMap<String, MediaType> {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let schema = self.to_schema(defmaker.ctx)?;
        Ok(InlineType::Json(Box::new(schema.inline(name, defmaker)?)))
    }
}

impl Inlining for RequestBody {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let inner = self.content.inline(name, defmaker)?;
        if self.required {
            Ok(inner)
        } else {
            Ok(InlineType::Option(Box::new(inner)))
        }
    }
}

fn status_to_string(status: &StatusCode) -> Result<String> {
    let code = match status {
        StatusCode::Code(value) => value,
        StatusCode::Range(_) => bail!("Could not convert range to int"),
    };

    let code_str = match code {
        100 => "CONTINUE",
        101 => "SWITCHING_PROTOCOLS",
        102 => "PROCESSING",
        300 => "MULTIPLE_CHOICES",
        301 => "MOVED_PERMANENTLY",
        302 => "FOUND",
        303 => "SEE_OTHER",
        304 => "NOT_MODIFIED",
        305 => "USE_PROXY",
        307 => "TEMPORARY_REDIRECT",
        308 => "PERMANENT_REDIRECT",
        400 => "BAD_REQUEST",
        401 => "UNAUTHORIZED",
        402 => "PAYMENT_REQUIRED",
        403 => "FORBIDDEN",
        404 => "NOT_FOUND",
        405 => "METHOD_NOT_ALLOWED",
        406 => "NOT_ACCEPTABLE",
        407 => "PROXY_AUTHENTICATION_REQUIRED",
        408 => "REQUEST_TIMEOUT",
        409 => "CONFLICT",
        410 => "GONE",
        411 => "LENGTH_REQUIRED",
        412 => "PRECONDITION_FAILED",
        413 => "PAYLOAD_TOO_LARGE",
        414 => "URI_TOO_LONG",
        415 => "UNSUPPORTED_MEDIA_TYPE",
        416 => "RANGE_NOT_SATISFIABLE",
        417 => "EXPECTATION_FAILED",
        418 => "IM_A_TEAPOT",
        421 => "MISDIRECTED_REQUEST",
        422 => "UNPROCESSABLE_ENTITY",
        423 => "LOCKED",
        424 => "FAILED_DEPENDENCY",
        426 => "UPGRADE_REQUIRED",
        428 => "PRECONDITION_REQUIRED",
        429 => "TOO_MANY_REQUESTS",
        431 => "REQUEST_HEADER_FIELDS_TOO_LARGE",
        451 => "UNAVAILABLE_FOR_LEGAL_REASONS",
        500 => "INTERNAL_SERVER_ERROR",
        501 => "NOT_IMPLEMENTED",
        502 => "BAD_GATEWAY",
        503 => "SERVICE_UNAVAILABLE",
        504 => "GATEWAY_TIMEOUT",
        505 => "HTTP_VERSION_NOT_SUPPORTED",
        506 => "VARIANT_ALSO_NEGOTIATES",
        507 => "INSUFFICIENT_STORAGE",
        508 => "LOOP_DETECTED",
        510 => "NOT_EXTENDED",
        511 => "NETWORK_AUTHENTICATION_REQUIRED",
        _ => bail!("Invalid error code"),
    }
    .to_string();

    Ok(code_str)
}

impl Inlining for IndexMap<&StatusCode, &ReferenceOr<Response>> {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let mut api_err_variants = Vec::new();

        for (status_code, response) in self {
            let status = status_to_string(status_code)?;

            let response = defmaker.ctx.deref(response)?;
            let schema = response.content.to_schema(defmaker.ctx)?;
            let schema = match &schema.schema_kind {
                SchemaKind::Type(value) => value,
                _ => bail!("Only concrete type schemas are implemented"),
            };
            let schema = match schema {
                Type::String(value) => value,
                _ => bail!("Error schemas must be string"),
            };

            if schema.enumeration.len() == 0 {
                bail!("Error schemas must contain enumeration")
            };

            for variant in &schema.enumeration {
                let Some(variant) = variant else {
                    bail!("Error enumeration must not contain null")
                };

                api_err_variants.push(ApiErrVariant {
                    name: variant.to_case(Case::UpperCamel),
                    detail: variant.clone(),
                    code: status.clone(),
                });
            }
        }

        let definition = Rc::new(Definition {
            name: name,
            data: DefinitionData::ApiErr(RApiErr {
                variants: api_err_variants,
            }),
        });
        defmaker.store.push(definition.clone());
        Ok(InlineType::Reference(definition))
    }
}

fn is_success(code: &StatusCode) -> bool {
    match code.deref() {
        StatusCode::Code(value) => value >= &200 && value < &300,
        StatusCode::Range(value) => value >= &200 && value < &300,
    }
}
/// Get success response code
/// If there is more that one success response, Returns an error
fn get_success_response<'a>(
    responses: &'a IndexMap<StatusCode, ReferenceOr<Response>>,
) -> Result<(&'a StatusCode, &'a ReferenceOr<Response>)> {
    let success_responses: Vec<(&StatusCode, &ReferenceOr<Response>)> = responses
        .iter()
        .filter(|(status_code, x)| is_success(status_code))
        .collect();

    let Some((success_status, success_response)) = success_responses.first() else {
        bail!("No success responses found")
    };

    if success_responses.len() > 1 {
        bail!("More that one success code found")
    };

    Ok((success_status, success_response))
}

fn get_error_responses<'a>(
    responses: &'a IndexMap<StatusCode, ReferenceOr<Response>>,
) -> IndexMap<&'a StatusCode, &'a ReferenceOr<Response>> {
    responses
        .iter()
        .filter(|(status_code, x)| !is_success(status_code))
        .collect()
}

impl Inlining for Response {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        self.content.inline(name, defmaker)
    }
}

impl Inlining for Responses {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        // Render success response
        let (success_response_code, success_response) = get_success_response(&self.responses)?;

        if success_response_code != &StatusCode::Code(200) {
            bail!("Only success code '200' supported")
        }

        let success_inline = success_response.inline(name.clone(), defmaker)?;

        // Render error responses
        let error_responses = get_error_responses(&self.responses);

        let res = if error_responses.len() > 0 {
            let err_inline = error_responses.inline(format!("{name}Error"), defmaker)?;
            InlineType::Result(Rc::new(success_inline), Rc::new(err_inline))
        } else {
            success_inline
        };

        Ok(res)
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
                        let deref = defmaker.ctx.deref_boxed(value)?;
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
            let prop_schema = defmaker.ctx.deref_boxed(prop_schema)?;
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
    Result(Rc<InlineType>, Rc<InlineType>),
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
            InlineType::Result(ok, err) => write!(f, "Result<{ok},{err}>"),
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
pub struct ApiErrVariant {
    name: String,   // Rust name of the variant
    detail: String, // How it is printed
    code: String,   // What is the code
}

/// Something that can serialize into api error
#[derive(Debug, Serialize)]
pub struct RApiErr {
    pub variants: Vec<ApiErrVariant>,
}

#[derive(Debug, Serialize)]
pub struct REnum {
    variants: Vec<String>,
}

#[derive(Debug, Serialize)]
pub enum DefinitionData {
    Struct(RStruct),
    Enum(REnum),
    ApiErr(RApiErr),
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
