use anyhow::{bail, Result};
use indexmap::IndexMap;
use openapiv3::{
    Components, MediaType, Parameter, ParameterData, ParameterSchemaOrContent, PathItem, PathStyle,
    QueryStyle, ReferenceOr, RequestBody, Response, Schema,
};

pub enum ParametersType {
    Query,
    Path,
    Header,
    Cookie,
}

pub trait ParameterStore {
    fn get_parameters_type() -> ParametersType;
}

impl ParameterStore for Vec<QueryParameter<'_>> {
    fn get_parameters_type() -> ParametersType {
        ParametersType::Query
    }
}

impl ParameterStore for Vec<PathParameter<'_>> {
    fn get_parameters_type() -> ParametersType {
        ParametersType::Path
    }
}

impl ParameterStore for Vec<HeaderParaemter<'_>> {
    fn get_parameters_type() -> ParametersType {
        ParametersType::Header
    }
}

impl ParameterStore for Vec<CookieParameter<'_>> {
    fn get_parameters_type() -> ParametersType {
        ParametersType::Cookie
    }
}

pub struct OpenApiCtx<'a> {
    components: &'a Option<Components>, // Used for dereferencing references
}

pub struct QueryParameter<'a> {
    pub parameter_data: &'a ParameterData,
    pub allow_reserved: &'a bool,
    pub style: &'a QueryStyle,
    pub allow_empty_value: &'a Option<bool>,
}

pub struct HeaderParaemter<'a> {
    pub parameter_data: &'a ParameterData,
}

pub struct PathParameter<'a> {
    pub parameter_data: &'a ParameterData,
    pub style: &'a PathStyle,
}

pub struct CookieParameter<'a> {
    pub parameter_data: &'a ParameterData,
}

pub struct ParametersSplitted<'a> {
    pub query_parameters: Vec<QueryParameter<'a>>,
    pub header_parameters: Vec<HeaderParaemter<'a>>,
    pub path_parameters: Vec<PathParameter<'a>>,
    pub cookie_parameters: Vec<CookieParameter<'a>>,
}

pub trait ToSchema {
    fn to_schema<'a>(&'a self, ctx: &OpenApiCtx<'a>) -> Result<&'a Schema>;
}

impl ToSchema for ParameterSchemaOrContent {
    fn to_schema<'a>(&'a self, ctx: &OpenApiCtx<'a>) -> Result<&'a Schema> {
        let schema = match self {
            openapiv3::ParameterSchemaOrContent::Schema(value) => ctx.deref(value)?,
            openapiv3::ParameterSchemaOrContent::Content(content) => content.to_schema(ctx)?,
        };
        Ok(schema)
    }
}

impl ToSchema for IndexMap<String, MediaType> {
    fn to_schema<'a>(&'a self, ctx: &OpenApiCtx<'a>) -> Result<&'a Schema> {
        if self.len() > 1 {
            bail!("Multiple content types for parameter are not supported")
        };
        let media = match self.get("application/json") {
            Some(value) => value,
            None => bail!("Only application/json content type is supported"),
        };
        let schema = match &media.schema {
            Some(value) => value,
            None => bail!("Content must have schema specified"),
        };
        ctx.deref(schema)
    }
}

impl ToSchema for RequestBody {
    fn to_schema<'a>(&'a self, ctx: &OpenApiCtx<'a>) -> Result<&'a Schema> {
        self.content.to_schema(ctx)
    }
}

pub trait Dereferencing<T> {
    fn dereference<'a>(components: &'a Components, namespace: &str, name: &str) -> Result<&'a T>;
}

fn verify_namespace(expected: &str, got: &str) -> Result<()> {
    if got != expected {
        bail!("Expected #/components/{expected} got #/components/{got}")
    }

    Ok(())
}

fn get_inner_reference<T>(ref_obj: &ReferenceOr<T>) -> Result<&T> {
    match ref_obj {
        ReferenceOr::Reference { reference: _ } => bail!("Reference in refernce not supported"),
        ReferenceOr::Item(value) => Ok(value),
    }
}

impl Dereferencing<Parameter> for Parameter {
    fn dereference<'a>(
        components: &'a Components,
        namespace: &str,
        name: &str,
    ) -> Result<&'a Parameter> {
        verify_namespace("parameters", namespace)?;

        let Some(value) = components.parameters.get(name) else {
            bail!("Reference not found")
        };

        // Just disallow nested top-level references to avoid circular dependencies
        let value = match value {
            ReferenceOr::Reference { reference: _ } => bail!("Reference in reference not allowed"),
            ReferenceOr::Item(value) => value,
        };

        Ok(value)
    }
}

impl Dereferencing<Schema> for Schema {
    fn dereference<'a>(
        components: &'a Components,
        namespace: &str,
        name: &str,
    ) -> Result<&'a Schema> {
        verify_namespace("schemas", namespace)?;

        let Some(value) = components.schemas.get(name) else {
            bail!("Reference not found")
        };

        get_inner_reference(value)
    }
}

impl Dereferencing<PathItem> for PathItem {
    fn dereference<'a>(
        _components: &'a Components,
        _namespace: &str,
        _name: &str,
    ) -> Result<&'a PathItem> {
        bail!("Referencing path items not supported");
    }
}

impl Dereferencing<Response> for Response {
    fn dereference<'a>(
        components: &'a Components,
        namespace: &str,
        name: &str,
    ) -> Result<&'a Response> {
        verify_namespace("responses", namespace)?;

        let Some(value) = components.responses.get(name) else {
            bail!("Reference not found")
        };

        get_inner_reference(value)
    }
}

impl Dereferencing<RequestBody> for RequestBody {
    fn dereference<'a>(
        components: &'a Components,
        namespace: &str,
        name: &str,
    ) -> Result<&'a RequestBody> {
        verify_namespace("requestBodies", namespace)?;

        let Some(value) = components.request_bodies.get(name) else {
            bail!("Reference not found")
        };

        get_inner_reference(value)
    }
}

fn deref_any<'a, T>(components: &'a Option<Components>, obj_ref: &str) -> Result<&'a T>
where
    T: Dereferencing<T>,
{
    let mut splitted = obj_ref.split('/');

    let (Some(hashsymbol),Some(comp),Some(namespace),Some(ref_name)) = (splitted.next(), splitted.next(), splitted.next(), splitted.next()) else {
        bail!("Invalid reference")
    };

    if hashsymbol != "#" {
        bail!("Reference must start with '#/'")
    };

    if comp != "components" {
        bail!("Reference must start with '#/components/'")
    }

    let Some(components) = components else {
        bail!("Reference found, but components are not specified")
    };

    T::dereference(components, namespace, ref_name)
}

impl<'a> OpenApiCtx<'a> {
    pub fn new(components: &'a Option<Components>) -> Self {
        OpenApiCtx { components }
    }

    pub fn deref_boxed<T>(&self, obj: &'a ReferenceOr<Box<T>>) -> Result<&'a T>
    where
        T: Dereferencing<T>,
    {
        let _obj_ref = match obj {
            ReferenceOr::Reference { reference } => reference,
            ReferenceOr::Item(value) => return Ok(value.as_ref()),
        };
        deref_any(self.components, _obj_ref)
    }

    /// Dereference openapi object
    pub fn deref<T>(&self, obj: &'a ReferenceOr<T>) -> Result<&'a T>
    where
        T: Dereferencing<T>,
    {
        let _obj_ref = match obj {
            ReferenceOr::Reference { reference } => reference,
            ReferenceOr::Item(value) => return Ok(value),
        };
        deref_any(self.components, _obj_ref)
    }

    pub fn split_parameters(
        &self,
        global_params: &'a [ReferenceOr<Parameter>],
        local_params: &'a [ReferenceOr<Parameter>],
    ) -> Result<ParametersSplitted<'a>> {
        let mut query_parameters = Vec::new();
        let mut header_parameters = Vec::new();
        let mut path_parameters = Vec::new();
        let mut cookie_parameters = Vec::new();

        for parameter in global_params.iter().chain(local_params.iter()) {
            let parameter_deref = self.deref(parameter)?;

            match parameter_deref {
                Parameter::Query {
                    parameter_data,
                    allow_reserved,
                    style,
                    allow_empty_value,
                } => query_parameters.push(QueryParameter {
                    parameter_data,
                    allow_reserved,
                    style,
                    allow_empty_value,
                }),
                Parameter::Header {
                    parameter_data,
                    style: _,
                } => header_parameters.push(HeaderParaemter { parameter_data }),
                Parameter::Path {
                    parameter_data,
                    style,
                } => path_parameters.push(PathParameter {
                    parameter_data,
                    style,
                }),
                Parameter::Cookie {
                    parameter_data,
                    style: _,
                } => cookie_parameters.push(CookieParameter { parameter_data }),
            }
        }

        Ok(ParametersSplitted {
            query_parameters,
            header_parameters,
            path_parameters,
            cookie_parameters,
        })
    }
}
