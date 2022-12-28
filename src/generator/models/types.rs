//! Type system that roughly maps to openapi type system

use std::{
    collections::{HashMap, HashSet},
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

use anyhow::{anyhow, bail, Context, Result};

use crate::{
    generator::templates::quote_str,
    openapictx::{
        CookieParameter, Dereferencing, HeaderParaemter, OpenApiCtx, ParameterStore,
        ParametersType, PathParameter, QueryParameter, ToSchema,
    },
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

            if schema.enumeration.is_empty() {
                bail!("Error schemas must contain enumeration")
            };

            for variant in &schema.enumeration {
                let Some(variant) = variant else {
                    bail!("Error enumeration must not contain null")
                };

                api_err_variants.push(ApiErrVariant {
                    name: to_rust_identifier(variant, Case::UpperCamel),
                    detail: variant.clone(),
                    code: status.clone(),
                });
            }
        }

        let definition = Rc::new(Definition {
            name,
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
        StatusCode::Code(value) => (&200..&300).contains(&value),
        StatusCode::Range(value) => (&200..&300).contains(&value),
    }
}
/// Get success response code
/// If there is more that one success response, Returns an error
fn get_success_response(
    responses: &IndexMap<StatusCode, ReferenceOr<Response>>,
) -> Result<(&StatusCode, &ReferenceOr<Response>)> {
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

fn get_error_responses(
    responses: &IndexMap<StatusCode, ReferenceOr<Response>>,
) -> IndexMap<&StatusCode, &ReferenceOr<Response>> {
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

        let res = if !error_responses.is_empty() {
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
        schema.inline(name, defmaker)
    }
}

fn render_parameter<T>(
    name: &String,
    param: &T,
    defmaker: &mut DefinitionMaker,
) -> Result<RStructProp>
where
    T: GenericParameter,
{
    let param_data = param.data();
    let inline_name = format!(
        "{}{}",
        name,
        to_rust_identifier(&param_data.name, Case::UpperCamel)
    );
    let inline = param.data().inline(inline_name, defmaker)?;

    let parameter_schema = param_data
        .format
        .to_schema(defmaker.ctx)
        .with_context(|| format!("Could not get parameter schema for {}", &param_data.name))?;

    let default = make_default_provider(&parameter_schema.schema_data.default, &inline, defmaker)?;

    validate_required_default_and_nullable(
        param_data.required,
        default.is_some(),
        parameter_schema.schema_data.nullable,
    )?;

    Ok(RStructProp {
        name: to_rust_identifier(&param.data().name, Case::Snake),
        rename: param.data().name.clone(),
        default,
        type_: inline,
    })
}

impl<T> MaybeInlining for Vec<T>
where
    T: GenericParameter,
    Vec<T>: ParameterStore,
{
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<Option<InlineType>> {
        if self.is_empty() {
            return Ok(None);
        }
        let mut properties = Vec::new();

        for param in self {
            properties
                .push(render_parameter(&name, param, defmaker).with_context(|| {
                    format!("Could not render parameter {}", param.data().name)
                })?);
        }

        let def = Rc::new(Definition {
            name,
            data: DefinitionData::Struct(RStruct {
                doc: None,
                properties,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        println!("{}", to_rust_identifier("12HelloWorld5", Case::Snake));
        println!("{}", to_rust_identifier("RGBArray", Case::Snake));
        println!("{}", to_rust_identifier("PostgreSQL 2341DB_", Case::Snake));
        println!("{}", to_rust_identifier("ThisMadWorld", Case::Snake));
        println!("{}", to_rust_identifier("helloMyDude", Case::Snake));
        println!("{}", to_rust_identifier("HELLO_WORLD", Case::Snake));
        println!("{}", to_rust_identifier("v1", Case::Snake));
        println!(
            "{}",
            "HelloWorldV1V3_RGBArray !@$5 :ADA:".to_case(Case::Title)
        );
    }
}

const RUST_KEYWORDS: &[&str] = &["match"];

pub fn to_rust_identifier(val: &str, case: Case) -> String {
    let val = slug::slugify(val.to_case(Case::Title));

    let mut result = val.from_case(Case::Kebab).to_case(case);

    if result.is_empty() {
        result = "_".to_string()
    }

    if let Some(value) = result.chars().into_iter().next() {
        if value.is_numeric() {
            result = format!("_{result}")
        }
    };

    if RUST_KEYWORDS.contains(&result.as_str()) {
        result = format!("{result}_");
    }

    result
}

fn enum_inline(
    name: String,
    defmaker: &mut DefinitionMaker,
    data: Vec<&String>,
    doc: &Option<String>,
) -> Result<InlineType> {
    let mut variants = Vec::new();
    for variant in data {
        let variant_value = (*variant).clone();
        variants.push(REnumVariant {
            name: to_rust_identifier(&variant_value, Case::UpperCamel),
            value: variant_value,
        })
    }
    let definition = Rc::new(Definition {
        name,
        data: DefinitionData::Enum(REnum {
            doc: doc.clone(),
            variants,
        }),
    });
    defmaker.store.push(definition.clone());
    Ok(InlineType::Reference(definition))
}

fn remove_options<T>(arr: &Vec<Option<T>>) -> Result<Vec<&T>> {
    let mut without_options = Vec::new();

    for val in arr {
        let val = val.as_ref().ok_or_else(|| anyhow!("Array contains null"))?;
        without_options.push(val);
    }

    Ok(without_options)
}

impl Inlining for Schema {
    fn inline(&self, name: String, defmaker: &mut DefinitionMaker) -> Result<InlineType> {
        let SchemaKind::Type(schema_type) = &self.schema_kind else {panic!("Only type schemas are implemented")};

        let mut type_ = match schema_type {
            Type::String(value) => {
                if value.enumeration.is_empty() {
                    InlineType::String
                } else {
                    let name = get_schema_name(name, &self.schema_data.title);
                    let variants = remove_options(&value.enumeration)
                        .context("Could not serialize enum variants")?;
                    enum_inline(name, defmaker, variants, &self.schema_data.description)?
                }
            }
            Type::Number(_) => InlineType::Float,
            Type::Integer(value) => InlineType::Integer,
            Type::Boolean {} => InlineType::Boolean,
            Type::Object(val) => {
                let name = get_schema_name(name, &self.schema_data.title);
                inline_obj(val, name, defmaker, &self.schema_data.description)?
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

        if self.schema_data.nullable {
            type_ = InlineType::Option(Box::new(type_))
        };

        Ok(type_)
    }
}

fn make_default_bool(val: &bool) -> (String, String) {
    let value = match val {
        true => "true",
        false => "false",
    }
    .to_string();
    let name = match val {
        true => "defaut_true",
        false => "default_false",
    }
    .to_string();
    (name, value)
}

fn make_default_int(val: &i64) -> (String, String) {
    (format!("default_int_{val}"), val.to_string())
}

fn make_default_float(val: &f64) -> (String, String) {
    (
        format!("default_float_{val}").replace('.', "_"),
        val.to_string(),
    )
}

fn make_default_str(val: &str) -> (String, String) {
    let name = format!("default_str_{}", to_rust_identifier(val, Case::Snake));
    (
        format!("default_str_{}", to_rust_identifier(val, Case::Snake)),
        format!("{}.to_string()", quote_str(val)),
    )
}

fn make_default_provider(
    val: &Option<serde_json::Value>,
    type_: &InlineType,
    defmaker: &mut DefinitionMaker,
) -> Result<Option<InlineType>> {
    let Some(val) = val else {
        return Ok(None)
    };

    let (inner_type, optional) = match type_ {
        InlineType::Option(inner) => (inner.as_ref(), true),
        _ => (type_, false),
    };

    let (mut name, mut value) = match val {
        serde_json::Value::Null => return Ok(None),
        serde_json::Value::Bool(value) => make_default_bool(value),
        serde_json::Value::Number(num) => match inner_type {
            InlineType::Integer => {
                if let Some(val) = num.as_i64() {
                    make_default_int(&val)
                } else {
                    bail!("Could not get default as i64")
                }
            }
            InlineType::Float => {
                if let Some(val) = num.as_f64() {
                    make_default_float(&val)
                } else {
                    bail!("Could not get default as f64")
                }
            }
            _ => bail!("Default is incompatible with the type: {:?}", inner_type),
        },
        serde_json::Value::String(value) => make_default_str(value),
        serde_json::Value::Array(_) => todo!(),
        serde_json::Value::Object(_) => todo!(),
    };

    if optional {
        value = format!("Some({value})");
        name = format!("opt_{name}")
    }

    // Look for duplicate providers and return them
    for def in &defmaker.store {
        let def_provider = match &def.data {
            DefinitionData::DefaultProvider(value) => value,
            _ => continue,
        };
        if def_provider.value == value && &def_provider.vtype == type_ {
            return Ok(Some(InlineType::Reference(def.clone())));
        }
    }

    let provider = DefaultProvider {
        vtype: type_.clone(),
        value,
    };

    // Or create new definition and return it
    let definition = Rc::new(Definition {
        name,
        data: DefinitionData::DefaultProvider(provider),
    });

    defmaker.store.push(definition.clone());

    Ok(Some(InlineType::Reference(definition)))
}

/// Openapi has 'required' 'default' and 'nullable' properties
/// Not all of them strictly map to rust serde
/// Some of the combinations of them are not possible/hard to implement
/// For example - what should happen if value is not required does not have default and is not nullable?
/// Or how value can be required and have default or nullable at the same time - that does not make much sense
///
/// required - when ture, value is physically required to be specified in json (even with null)
/// has_default - when true, the value has default value
/// is_nullable - when true - the value can be nullable
fn validate_required_default_and_nullable(
    required: bool,
    has_default: bool,
    is_nullable: bool,
) -> Result<()> {
    match (required, has_default, is_nullable) {
        (true, true, _) => bail!("Value cannot be required and have default at the same time"),
        (true, false, false) => Ok(()), // Value is required and does not have default
        (true, false, true) => bail!("Value cannot be required and be nullable at the same time"),
        (false, true, _) => Ok(()), // Values are not required and have default
        (false, false, true) => Ok(()), // Value is not required, does not have default but is nullable
        (false, false, false) => {
            bail!("Value is not required, does not have default and is nullable at the same time")
        }
    }
}

fn inline_obj(
    obj: &ObjectType,
    name: String,
    defmaker: &mut DefinitionMaker,
    doc: &Option<String>,
) -> Result<InlineType> {
    let mut properties = Vec::new();

    let required: HashSet<&String> = obj.required.iter().collect();

    for (prop_name, prop_schema) in obj.properties.iter() {
        let prop_schema = defmaker
            .ctx
            .deref_boxed(prop_schema)
            .with_context(|| format!("Could not dereference {prop_name}"))?;

        let prop_name_camel = to_rust_identifier(prop_name, Case::UpperCamel);

        let type_ = prop_schema
            .inline(format!("{name}{prop_name_camel}"), defmaker)
            .with_context(|| format!("Could not make inline type for {prop_name}"))?;

        let default = make_default_provider(&prop_schema.schema_data.default, &type_, defmaker)
            .with_context(|| format!("Could not make default value for {prop_name}"))?;

        let prop_required = required.contains(prop_name);

        validate_required_default_and_nullable(
            prop_required,
            default.is_some(),
            prop_schema.schema_data.nullable,
        )
        .with_context(|| format!("Could not validate required and nullable for {prop_name}"))?;

        properties.push(RStructProp {
            name: to_rust_identifier(prop_name, Case::Snake),
            rename: prop_name.clone(),
            default,
            type_: type_,
        })
    }

    let definition = Rc::new(Definition {
        name,
        data: DefinitionData::Struct(RStruct {
            doc: doc.clone(),
            properties,
        }),
    });

    defmaker.store.push(definition.clone());

    Ok(InlineType::Reference(definition))
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
#[derive(Debug, Clone)]
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

impl PartialEq for InlineType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Array(l0), Self::Array(r0)) => l0 == r0,
            (Self::Map(l0), Self::Map(r0)) => l0 == r0,
            (Self::Json(l0), Self::Json(r0)) => l0 == r0,
            (Self::Path(l0), Self::Path(r0)) => l0 == r0,
            (Self::Query(l0), Self::Query(r0)) => l0 == r0,
            (Self::Option(l0), Self::Option(r0)) => l0 == r0,
            (Self::Reference(l0), Self::Reference(r0)) => l0.name == r0.name,
            (Self::Result(l0, l1), Self::Result(r0, r1)) => l0 == r0 && l1 == r1,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl Eq for InlineType {}

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
#[derive(Debug, Serialize)]
pub struct RStructProp {
    pub name: String,
    pub rename: String,
    pub default: Option<InlineType>,
    pub type_: InlineType,
}

/// Something that can serialize into rust struct
#[derive(Debug, Serialize)]
pub struct RStruct {
    pub doc: Option<String>,
    pub properties: Vec<RStructProp>,
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
pub struct REnumVariant {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct REnum {
    pub doc: Option<String>,
    pub variants: Vec<REnumVariant>,
}

#[derive(Debug, Serialize)]
pub struct DefaultProvider {
    pub vtype: InlineType,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub enum DefinitionData {
    Struct(RStruct),
    Enum(REnum),
    ApiErr(RApiErr),
    DefaultProvider(DefaultProvider),
}

#[derive(Debug, Serialize)]
pub struct Definition {
    pub name: String,
    pub data: DefinitionData,
}

fn get_schema_name(name: String, title: &Option<String>) -> String {
    if let Some(val) = title {
        val.clone()
    } else {
        name
    }
}
