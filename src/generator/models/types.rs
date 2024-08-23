//! Type system that roughly maps to openapi type system

use std::{
    collections::HashSet,
    fmt::{Debug, Display},
    ops::Deref,
};

use convert_case::{Case, Casing};
use indexmap::IndexMap;
use openapiv3::{
    MediaType, ObjectType, ParameterData, ReferenceOr, RequestBody, Response, Responses, Schema,
    SchemaData, SchemaKind, StatusCode, Type,
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
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<Option<InlineType>>;
}

impl<T> MaybeInlining for Option<T>
where
    T: Inlining,
{
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<Option<InlineType>> {
        match self {
            Some(value) => Ok(Some(value.inline(name, version, ctx, defmaker)?)),
            None => Ok(None),
        }
    }
}

impl<T> MaybeInlining for ReferenceOr<T>
where
    T: MaybeInlining + Dereferencing<T>,
{
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<Option<InlineType>> {
        let deref = ctx.deref(self)?;
        deref.inline(name, version, ctx, defmaker)
    }
}

pub trait Inlining {
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType>;
}

impl<T> Inlining for ReferenceOr<T>
where
    T: Inlining + Dereferencing<T>,
{
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType> {
        let deref = ctx.deref(self)?;
        deref.inline(name, version, ctx, defmaker)
    }
}

impl Inlining for IndexMap<String, MediaType> {
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType> {
        let schema = self.to_schema(ctx)?;
        Ok(InlineType::Json(Box::new(
            schema.inline(name, version, ctx, defmaker)?,
        )))
    }
}

impl Inlining for RequestBody {
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType> {
        let inner = self.content.inline(name, version, ctx, defmaker)?;
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
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType> {
        let mut api_err_variants = Vec::new();
        let mut doc_vec = Vec::new();
        for (status_code, response) in self {
            let status = status_to_string(status_code)?;
            let response = ctx.deref(response)?;
            let schema = response.content.to_schema(ctx)?;
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

            doc_vec.push(format!(
                "Status {}:\n{}",
                status.clone(),
                response.description
            ));

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
        let definition = Definition {
            data: DefinitionData::ApiErr(RApiErr {
                doc: Some(doc_vec.join("\n\n")),
                variants: api_err_variants,
            }),
        };
        let definition = defmaker.push(name, version, definition)?;
        Ok(InlineType::Reference(definition))
    }
}

fn is_success(code: &StatusCode) -> bool {
    match code {
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
        .filter(|(status_code, _)| is_success(status_code))
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
        .filter(|(status_code, _)| !is_success(status_code))
        .collect()
}

impl Inlining for Response {
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType> {
        self.content.inline(name, version, ctx, defmaker)
    }
}

impl Inlining for Responses {
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType> {
        // Render success response
        let (success_response_code, success_response) = get_success_response(&self.responses)?;

        if success_response_code != &StatusCode::Code(200) {
            bail!("Only success code '200' supported")
        }

        let success_inline = success_response.inline(name.clone(), version, ctx, defmaker)?;

        // Render error responses
        let error_responses = get_error_responses(&self.responses);

        let res = if !error_responses.is_empty() {
            let err_inline =
                error_responses.inline(format!("{name}Error"), version, ctx, defmaker)?;
            InlineType::Result(
                Box::new(success_inline),
                Box::new(InlineType::Detailed(Box::new(err_inline))),
            )
        } else {
            success_inline
        };

        Ok(res)
    }
}

impl Inlining for ParameterData {
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType> {
        let schema = self.format.to_schema(ctx)?;
        schema.inline(name, version, ctx, defmaker)
    }
}

fn render_parameter<T>(
    name: &String,
    version: usize,
    param: &T,
    ctx: &OpenApiCtx<'_>,
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
    let inline = param.data().inline(inline_name, version, ctx, defmaker)?;

    let parameter_schema = param_data
        .format
        .to_schema(ctx)
        .with_context(|| format!("Could not get parameter schema for {}", &param_data.name))?;

    let default = make_default_provider(
        version,
        &parameter_schema.schema_data.default,
        &inline,
        defmaker,
    )?;

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
        doc: param_data.description.clone(),
    })
}

impl<T> MaybeInlining for Vec<T>
where
    T: GenericParameter,
    Vec<T>: ParameterStore,
{
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<Option<InlineType>> {
        if self.is_empty() {
            return Ok(None);
        }
        let mut properties = Vec::new();

        for param in self {
            properties.push(
                render_parameter(&name, version, param, ctx, defmaker)
                    .with_context(|| format!("Could not render parameter {}", param.data().name))?,
            );
        }

        let def = Definition {
            data: DefinitionData::Struct(RStruct {
                doc: None,
                properties,
            }),
        };

        let def = defmaker.push(name, version, def)?;

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

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

pub fn to_rust_identifier(val: &str, case: Case) -> String {
    let val = slug::slugify(val.to_case(Case::Title));

    let mut result = val.from_case(Case::Kebab).to_case(case);

    if result.is_empty() {
        result = "_".to_string()
    }

    if let Some(value) = result.chars().next() {
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
    version: usize,
    defmaker: &mut DefinitionMaker,
    data: Vec<&String>,
    doc: &Option<String>,
) -> Result<InlineType> {
    let mut variants = Vec::new();
    for variant in data {
        let variant_value = (*variant).clone();
        variants.push(REnumVariant {
            name: to_rust_identifier(&variant_value, Case::UpperCamel),
            rename: variant_value,
            data: None,
        })
    }
    let definition = Definition {
        data: DefinitionData::Enum(REnum {
            doc: doc.clone(),
            variants,
            discriminator: None,
        }),
    };
    let definition = defmaker.push(name, version, definition)?;
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

fn schema_type_to_inline_type(
    name: String,
    version: usize,
    ctx: &OpenApiCtx<'_>,
    defmaker: &mut DefinitionMaker,
    schema_type: &Type,
    schema_data: &SchemaData,
) -> Result<InlineType> {
    let mut type_ = match schema_type {
        Type::String(value) => {
            if value.enumeration.is_empty() {
                InlineType::String
            } else {
                let name = get_schema_name(name, &schema_data.title);
                let variants = remove_options(&value.enumeration)
                    .context("Could not serialize enum variants")?;
                enum_inline(name, version, defmaker, variants, &schema_data.description)?
            }
        }
        Type::Number(_) => InlineType::Float,
        Type::Integer(_) => InlineType::Integer,
        Type::Boolean {} => InlineType::Boolean,
        Type::Object(val) => {
            let name = get_schema_name(name, &schema_data.title);
            inline_obj(val, name, version, ctx, defmaker, &schema_data.description)?
        }
        Type::Array(val) => {
            let new_inline = match &val.items {
                Some(value) => {
                    let deref = ctx.deref_boxed(value)?;
                    deref.inline(format!("{name}Item"), version, ctx, defmaker)?
                }
                None => InlineType::Any,
            };
            InlineType::Array(Box::new(new_inline))
        }
    };

    if schema_data.nullable {
        type_ = InlineType::Option(Box::new(type_))
    };

    Ok(type_)
}

fn get_discriminator_prop(
    schema_orig: &Schema,
    discriminator: &String,
    ctx: &OpenApiCtx<'_>,
) -> Result<(String, Schema)> {
    let SchemaKind::Type(ref schema) = schema_orig.schema_kind else {
        bail!("Only object can have discriminator property")
    };

    let Type::Object(schema) = schema else {
        bail!("Only object can have discriminator property")
    };

    let Some(discriminator_prop) = schema.properties.get(discriminator) else {
        bail!("Could not find discriminator property")
    };

    let mut schema = schema.clone();
    schema.properties.remove(discriminator);
    let schema_ret = Schema {
        schema_data: schema_orig.schema_data.clone(),
        schema_kind: SchemaKind::Type(Type::Object(schema)),
    };
    let discriminator_prop = ctx.deref_boxed(discriminator_prop)?;

    let SchemaKind::Type(ref discriminator_prop) = discriminator_prop.schema_kind else {
        bail!("Only concrete types are supported as discriminators")
    };

    let Type::String(discriminator_prop) = discriminator_prop else {
        bail!("Discriminator property must be string")
    };

    let Some(discriminator_value) = discriminator_prop.enumeration.first() else {
        bail!("Discriminator property must contain enumeration")
    };

    if discriminator_prop.enumeration.len() > 1 {
        bail!("Discriminator property must have exactly one enumeration value")
    }

    let Some(discriminator_value) = discriminator_value else {
        bail!("Discriminator property must have exactly one enumeration value that is not null")
    };

    Ok((discriminator_value.clone(), schema_ret))
}

fn discriminator_property(discriminator: &openapiv3::Discriminator) -> Result<String> {
    if !discriminator.extensions.is_empty() {
        bail!("Discriminator extensions not supported")
    }
    if !discriminator.mapping.is_empty() {
        bail!("Discriminator mapping not supported")
    }
    Ok(discriminator.property_name.clone())
}

fn one_of_to_inline_type(
    name: String,
    version: usize,
    ctx: &OpenApiCtx<'_>,
    defmaker: &mut DefinitionMaker,
    schemas: Vec<&Schema>,
    discriminator: &Option<openapiv3::Discriminator>,
    doc: &Option<String>,
) -> Result<InlineType> {
    let mut variants = Vec::new();

    let discriminator = match discriminator {
        Some(discriminator) => {
            let discriminator = discriminator_property(discriminator)?;
            for schema in schemas {
                let (variant_name, schema) = get_discriminator_prop(schema, &discriminator, ctx)?;

                let schema_inlined = schema
                    .inline(
                        to_rust_identifier(
                            &format!("{} {}", &name, &variant_name),
                            Case::UpperCamel,
                        ),
                        version,
                        ctx,
                        defmaker,
                    )
                    .with_context(|| format!("Could process anyOf {}", variant_name))?;

                variants.push(REnumVariant {
                    name: to_rust_identifier(&variant_name, Case::UpperCamel),
                    rename: variant_name.clone(),
                    data: Some(schema_inlined),
                });
            }
            Some(discriminator)
        }
        None => {
            bail!("oneOf without discriminator not supported")
        }
    };

    let definition = defmaker.push(
        name,
        version,
        Definition {
            data: DefinitionData::Enum(REnum {
                doc: doc.clone(),
                variants,
                discriminator,
            }),
        },
    )?;

    Ok(InlineType::Reference(definition))
}

impl Inlining for Schema {
    fn inline(
        &self,
        name: String,
        version: usize,
        ctx: &OpenApiCtx<'_>,
        defmaker: &mut DefinitionMaker,
    ) -> Result<InlineType> {
        match &self.schema_kind {
            SchemaKind::Type(schema_type) => schema_type_to_inline_type(
                name,
                version,
                ctx,
                defmaker,
                schema_type,
                &self.schema_data,
            ),
            SchemaKind::OneOf { one_of } => {
                let mut schemas = Vec::new();
                for schema in one_of {
                    let schema = ctx.deref(schema)?;
                    schemas.push(schema);
                }

                if self.schema_data.discriminator.is_none() {
                    bail!("Discriminator is None!")
                };

                one_of_to_inline_type(
                    name,
                    version,
                    ctx,
                    defmaker,
                    schemas,
                    &self.schema_data.discriminator,
                    &self.schema_data.description,
                )
            }
            SchemaKind::AnyOf { any_of: _ } => bail!("Serializing 'anyOf' not supported"),
            SchemaKind::AllOf { all_of: _ } => bail!("Serializing 'allOf' not supported"),
            SchemaKind::Not { not: _ } => bail!("Serializing 'not' not supported"),
            SchemaKind::Any(_value) => {
                bail!("Could not understand openapi object")
            }
        }
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
    (
        format!("default_str_{}", to_rust_identifier(val, Case::Snake)),
        format!("{}.to_string()", quote_str(val)),
    )
}

fn make_default_provider(
    version: usize,
    val: &Option<serde_json::Value>,
    type_: &InlineType,
    defmaker: &mut DefinitionMaker,
) -> Result<Option<InlineType>> {
    let Some(val) = val else { return Ok(None) };

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

    let provider = DefaultProvider {
        vtype: type_.clone(),
        value,
    };

    let definition = Definition {
        data: DefinitionData::DefaultProvider(provider),
    };

    let definition = defmaker.push(name, version, definition)?;

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
            bail!(
                "Value is not required, does not have default and is not nullable at the same time"
            )
        }
    }
}

fn inline_obj(
    obj: &ObjectType,
    name: String,
    version: usize,
    ctx: &OpenApiCtx<'_>,
    defmaker: &mut DefinitionMaker,
    doc: &Option<String>,
) -> Result<InlineType> {
    let mut properties = Vec::new();

    let required: HashSet<&String> = obj.required.iter().collect();

    for (prop_name, prop_schema) in obj.properties.iter() {
        let prop_schema = ctx
            .deref_boxed(prop_schema)
            .with_context(|| format!("Could not dereference {prop_name}"))?;

        let prop_name_camel = to_rust_identifier(prop_name, Case::UpperCamel);

        let type_ = prop_schema
            .inline(format!("{name}{prop_name_camel}"), version, ctx, defmaker)
            .with_context(|| format!("Could not make inline type for {prop_name}"))?;

        let default =
            make_default_provider(version, &prop_schema.schema_data.default, &type_, defmaker)
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
            type_,
            doc: prop_schema.schema_data.description.clone(),
        })
    }

    let definition = Definition {
        data: DefinitionData::Struct(RStruct {
            doc: doc.clone(),
            properties,
        }),
    };

    let definition = defmaker.push(name, version, definition)?;

    Ok(InlineType::Reference(definition))
}

/// Http method
#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Serialize)]
pub enum HttpMethod {
    Post,
    Get,
    Delete,
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Post => write!(f, "post"),
            HttpMethod::Get => write!(f, "get"),
            HttpMethod::Delete => write!(f, "delete"),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct OperationPath {
    pub operation: String,
    pub path: String,
    pub method: HttpMethod, // Operation method
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RustOperation {
    pub doc: Option<String>,
    pub param_path: Option<InlineType>,  // web::Path
    pub param_query: Option<InlineType>, // web::Query
    pub param_body: Option<InlineType>,  // web::Json

    // Response
    // -----------------------------
    pub response: InlineType,
}

// pub ctx: &'a OpenApiCtx<'a>,

pub struct DefinitionMaker<'a, 'b> {
    pub dedup_store: &'a mut IndexMap<String, Definition>,
    pub operations: &'b mut IndexMap<String, RustOperation>,
}

impl<'a, 'b> DefinitionMaker<'a, 'b> {
    pub fn new(
        store: &'a mut IndexMap<String, Definition>,
        operations: &'b mut IndexMap<String, RustOperation>,
    ) -> Self {
        DefinitionMaker {
            dedup_store: store,
            operations,
        }
    }

    pub fn push(&mut self, mut name: String, version: usize, def: Definition) -> Result<String> {
        for (existing_def_name, existing_def) in &*self.dedup_store {
            if &def == existing_def {
                return Ok(existing_def_name.clone());
            }
        }

        // Add version prefix if value already exists
        if self.dedup_store.contains_key(&name) {
            name = match def.data {
                DefinitionData::DefaultProvider(_) => format!("{}_v{}", name, version),
                DefinitionData::StaticHtmlPath(_) => format!("{}_v{}", name, version),
                DefinitionData::StaticStringPath(_) => format!("{}_v{}", name, version),
                DefinitionData::Redirect(_) => format!("{}_v{}", name, version),
                _ => format!("{}V{}", name, version),
            };
        }

        if self.dedup_store.insert(name.clone(), def).is_some() {
            bail!("Duplicate definition name {name}")
        }

        Ok(name)
    }

    pub fn push_operation(
        &mut self,
        mut name: String,
        version: usize,
        op: RustOperation,
    ) -> Result<String> {
        for (existing_op_name, existing_op) in &*self.operations {
            if &op == existing_op {
                return Ok(existing_op_name.clone());
            }
        }

        if self.operations.contains_key(&name) {
            name = format!("{}_v{}", name, version)
        };

        if self.operations.insert(name.clone(), op).is_some() {
            bail!("Duplicate operation name {name}")
        }

        Ok(name)
    }
}

/// Arbitrary inline type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineType {
    String,
    Integer,
    Float,
    Boolean,
    Any,
    Array(Box<InlineType>),  // Vec::<InlineType>
    Json(Box<InlineType>),   // web::Json
    Path(Box<InlineType>),   // web::Path
    Query(Box<InlineType>),  // web::Query
    Option(Box<InlineType>), // Option<InlineType>
    Reference(String),
    Result(Box<InlineType>, Box<InlineType>),
    Detailed(Box<InlineType>),
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
            InlineType::Json(item) => write!(f, "web::Json<{item}>"),
            InlineType::Path(item) => write!(f, "web::Path<{item}>"),
            InlineType::Query(item) => write!(f, "web::Query<{item}>"),
            InlineType::Option(item) => write!(f, "Option<{item}>"),
            InlineType::Reference(item) => Display::fmt(&item, f),
            InlineType::Result(ok, err) => write!(f, "Result<{ok}, {err}>"),
            InlineType::Detailed(item) => write!(f, "Detailed<{item}>"),
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
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RStructProp {
    pub name: String,
    pub rename: String,
    pub default: Option<InlineType>,
    pub type_: InlineType,
    pub doc: Option<String>,
}

/// Something that can serialize into rust struct
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RStruct {
    pub doc: Option<String>,
    pub properties: Vec<RStructProp>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ApiErrVariant {
    pub name: String,   // Rust name of the variant
    pub detail: String, // How it is printed
    pub code: String,   // What is the code
}

/// Something that can serialize into api error
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RApiErr {
    pub doc: Option<String>,
    pub variants: Vec<ApiErrVariant>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct REnumVariant {
    pub name: String,
    pub rename: String,
    pub data: Option<InlineType>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct REnum {
    pub doc: Option<String>,
    pub variants: Vec<REnumVariant>,
    pub discriminator: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DefaultProvider {
    pub vtype: InlineType,
    pub value: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StaticStr {
    pub path: String,
}

/// Serves static string on given path
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StaticStringPath {
    pub data: String,
}

/// Serves static html on given path
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StaticHtmlPath {
    pub data: String,
}

/// Serves static html on given path
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StaticRedirect {
    pub target: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum DefinitionData {
    Struct(RStruct),
    Enum(REnum),
    ApiErr(RApiErr),
    DefaultProvider(DefaultProvider),
    StaticStr(StaticStr),
    StaticStringPath(StaticStringPath),
    StaticHtmlPath(StaticHtmlPath),
    Redirect(StaticRedirect),
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Definition {
    pub data: DefinitionData,
}

/// Get name for schema
fn get_schema_name(name: String, title: &Option<String>) -> String {
    if let Some(val) = title {
        to_rust_identifier(val, Case::UpperCamel)
    } else {
        name
    }
}
