use anyhow::{bail, Result};
use indexmap::IndexMap;
use openapiv3::{
    Components, MediaType, Parameter, ParameterData, ParameterSchemaOrContent, PathStyle,
    QueryStyle, ReferenceOr, RequestBody, Schema,
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
            openapiv3::ParameterSchemaOrContent::Schema(value) => ctx.deref(value),
            openapiv3::ParameterSchemaOrContent::Content(content) => content.to_schema(ctx)?,
        };
        Ok(schema)
    }
}

impl ToSchema for IndexMap<String, MediaType> {
    fn to_schema<'a>(&'a self, ctx: &OpenApiCtx<'a>) -> Result<&'a Schema> {
        if self.len() > 1 {
            bail!("Content types for parameter are not supported")
        };
        let media = match self.get("application/json") {
            Some(value) => value,
            None => bail!("Only application/json content type is supported"),
        };
        let schema = match &media.schema {
            Some(value) => value,
            None => bail!("Content must have schema specified"),
        };
        Ok(ctx.deref(schema))
    }
}

impl ToSchema for RequestBody {
    fn to_schema<'a>(&'a self, ctx: &OpenApiCtx<'a>) -> Result<&'a Schema> {
        self.content.to_schema(ctx)
    }
}

impl<'a> OpenApiCtx<'a> {
    pub fn new(components: &'a Option<Components>) -> Self {
        OpenApiCtx { components }
    }

    /// Dereference openapi object
    pub fn deref<T>(&self, obj: &'a ReferenceOr<T>) -> &'a T {
        let _obj_ref = match obj {
            ReferenceOr::Reference { reference } => reference,
            ReferenceOr::Item(value) => return value,
        };
        todo!()
    }

    pub fn split_parameters(
        &self,
        global_params: &'a Vec<ReferenceOr<Parameter>>,
        local_params: &'a Vec<ReferenceOr<Parameter>>,
    ) -> ParametersSplitted<'a> {
        let mut query_parameters = Vec::new();
        let mut header_parameters = Vec::new();
        let mut path_parameters = Vec::new();
        let mut cookie_parameters = Vec::new();

        for parameter in global_params.iter().chain(local_params.iter()) {
            let parameter_deref = self.deref(parameter);

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

        ParametersSplitted {
            query_parameters,
            header_parameters,
            path_parameters,
            cookie_parameters,
        }
    }
}
