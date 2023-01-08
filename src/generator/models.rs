use convert_case::Case;
use indexmap::{IndexMap, IndexSet};
use openapiv3::{OpenAPI, Parameter, ReferenceOr};
use serde::Serialize;
pub mod types;
use anyhow::{bail, Context, Result};

use crate::openapictx::OpenApiCtx;

use self::types::{
    to_rust_identifier, Definition, DefinitionMaker, HttpMethod, Inlining, MaybeInlining,
    OperationPath, RustOperation, StaticHtmlPath, StaticRedirect, StaticStr, StaticStringPath,
};

/// Reference to ApiErr definition
#[derive(Debug, Serialize)]
pub struct ApiErrRef(pub String);

#[derive(Debug, Serialize)]
pub struct StaticService {
    pub method: HttpMethod,
    pub path: String,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct ApiService {
    pub definitions: IndexMap<String, Definition>,
    pub operations: IndexMap<String, RustOperation>,
    pub paths: Vec<OperationPath>,
    /// Paths to openapi specs
    pub static_services: Vec<StaticService>,
}

#[derive(Debug, Serialize)]
pub struct RustModule {
    pub api: ApiService,
}

fn to_operation_map(
    path_item: &openapiv3::PathItem,
) -> IndexMap<HttpMethod, &openapiv3::Operation> {
    let mut result = IndexMap::new();

    if let Some(op) = &path_item.get {
        result.insert(HttpMethod::Get, op);
    }

    if let Some(op) = &path_item.post {
        result.insert(HttpMethod::Post, op);
    }

    if let Some(op) = &path_item.delete {
        result.insert(HttpMethod::Delete, op);
    }

    result
}

fn to_rust_operation(
    ctx: &OpenApiCtx,
    defmaker: &mut DefinitionMaker,
    path: &str,
    method: HttpMethod,
    operation: &openapiv3::Operation,
    global_params: &[ReferenceOr<Parameter>],
    version: usize,
) -> Result<Vec<OperationPath>> {
    // Get operation name
    let Some(name) = &operation.operation_id else {
        bail!("Operation must have operation_id")
    };

    let name_upper = to_rust_identifier(name, Case::UpperCamel);

    // Get operation docstring
    let doc = operation
        .summary
        .as_ref()
        .or(operation.description.as_ref())
        .cloned();

    let params_spliited = ctx
        .split_parameters(global_params, &operation.parameters)
        .context("Could not split parameters")?;

    let path_params_inline = params_spliited
        .path_parameters
        .inline(format!("{name_upper}Path"), version, ctx, defmaker)
        .context("Could not inline path parameters")?;

    let query_params_inline = params_spliited
        .query_parameters
        .inline(format!("{name_upper}Query"), version, ctx, defmaker)
        .context("Could not inline query parameters")?;

    if !params_spliited.header_parameters.is_empty() {
        bail!("Header parameters not supported")
    };

    if !params_spliited.cookie_parameters.is_empty() {
        bail!("Cookie parameters not supported")
    };

    let param_body = operation
        .request_body
        .inline(format!("{name_upper}Body"), version, ctx, defmaker)
        .context("Could not inline Body")?;

    let response = operation
        .responses
        .inline(name_upper, version, ctx, defmaker)
        .context("Could not inline response")?;

    let operation = RustOperation {
        // name: name.clone(),
        // method,
        doc,
        param_path: path_params_inline,
        param_query: query_params_inline,
        param_body,

        // Response
        // -----------------------------
        response,
    };

    let operation = defmaker.push_operation(name.clone(), version, operation)?;

    let mut paths = Vec::new();

    if version == 1 {
        // Push path without prefix for version 1
        paths.push(OperationPath {
            operation: operation.clone(),
            method,
            path: path.to_string(),
        });
    }

    paths.push(OperationPath {
        operation,
        method,
        path: format!("/v{version}{path}"),
    });

    Ok(paths)
}

pub fn to_openapi_site(
    version: usize,
    path: String,
    path_html: String,
    path_openapi: String,
    defmaker: &mut DefinitionMaker,
) -> Result<Vec<StaticService>> {
    let mut services = Vec::new();

    let openapi_static = defmaker.push(
        "DOCS_OPENAPI".to_string(),
        version,
        Definition {
            data: types::DefinitionData::StaticStr(StaticStr { path: path_openapi }),
        },
    )?;

    let docs_static = defmaker.push(
        "DOCS_HTML".to_string(),
        version,
        Definition {
            data: types::DefinitionData::StaticStr(StaticStr { path: path_html }),
        },
    )?;

    let openapi = defmaker.push(
        "openapi".to_string(),
        version,
        Definition {
            data: types::DefinitionData::StaticStringPath(StaticStringPath {
                data: openapi_static,
            }),
        },
    )?;

    services.push(StaticService {
        method: HttpMethod::Get,
        path: format!("{path}/openapi.yaml"),
        data: openapi,
    });

    let docs = defmaker.push(
        "docs".to_string(),
        version,
        Definition {
            data: types::DefinitionData::StaticHtmlPath(StaticHtmlPath { data: docs_static }),
        },
    )?;

    services.push(StaticService {
        method: HttpMethod::Get,
        path: format!("{path}/docs"),
        data: docs,
    });

    Ok(services)
}

pub struct OpenApiWithPath {
    pub spec_path: String,
    pub spec: OpenAPI,
}

pub fn extract_major_from_version(version: &str) -> Result<usize> {
    let mut version_elements = version.split('.');

    let Some(major) = version_elements.next() else {
        bail!("Could not understand major from string {:?}",version);
    };
    let major: usize = major
        .parse()
        .with_context(|| format!("Could not get major as usize from {:?}", version))?;
    Ok(major)
}

fn add_redirect(
    name: String,
    version: usize,
    path: &str,
    target: &str,
    defmaker: &mut DefinitionMaker,
) -> Result<StaticService> {
    let redirect: StaticRedirect = StaticRedirect {
        target: target.to_string(),
    };

    let operation = defmaker.push(
        name,
        version,
        types::Definition {
            data: types::DefinitionData::Redirect(redirect),
        },
    )?;

    Ok(StaticService {
        data: operation,
        path: path.to_string(),
        method: HttpMethod::Get,
    })
}

pub fn to_rust_module(doc_path: &str, specs: &[OpenApiWithPath]) -> Result<RustModule> {
    let mut operations = IndexMap::new();
    let mut paths = Vec::new();
    let mut static_services = Vec::new();

    let mut definitions = IndexMap::new();

    let mut seen_version = IndexSet::new();

    let mut defmaker = DefinitionMaker::new(&mut definitions, &mut operations);

    for OpenApiWithPath { spec, spec_path } in specs {
        let ctx = OpenApiCtx::new(&spec.components);

        let version =
            extract_major_from_version(&spec.info.version).context("Could not get spec version")?;

        if !seen_version.insert(version) {
            bail!("Duplicate openapi version: {version}")
        }

        if version == 1 {
            static_services.extend(to_openapi_site(
                version,
                "".to_string(),
                doc_path.to_string(),
                spec_path.clone(),
                &mut defmaker,
            )?);
        }

        static_services.push(add_redirect(
            format!("to_v{version}_docs"),
            version,
            &format!("/v{version}"),
            &format!("v{version}/docs"),
            &mut defmaker,
        )?);

        static_services.push(add_redirect(
            "to_docs".to_string(),
            version,
            &format!("/v{version}/"),
            "docs",
            &mut defmaker,
        )?);

        static_services.extend(to_openapi_site(
            version,
            format!("/v{version}"),
            doc_path.to_string(),
            spec_path.clone(),
            &mut defmaker,
        )?);

        for (path, path_item) in spec.paths.iter() {
            let path_item = ctx.deref(path_item)?;
            let global_params: &Vec<ReferenceOr<Parameter>> = &path_item.parameters;
            for (method, operation) in to_operation_map(path_item) {
                let operation_paths = to_rust_operation(
                    &ctx,
                    &mut defmaker,
                    path,
                    method,
                    operation,
                    global_params,
                    version,
                )
                .with_context(|| {
                    format!(
                        "Could not convert to rust operation at {} {}",
                        &method, &path
                    )
                })?;

                for operation_path in operation_paths {
                    if !paths.contains(&operation_path) {
                        paths.push(operation_path);
                    }
                }
            }
        }
    }

    let Some(latest_version) = seen_version.iter().max().cloned() else {
        bail!("Could not determine latest version to redirect to")
    };

    // Add redirect to latest docs
    if latest_version == 1 {
        static_services.push(add_redirect(
            "to_docs".to_string(),
            latest_version,
            "/",
            "docs",
            &mut defmaker,
        )?);
    } else {
        static_services.push(add_redirect(
            format!("to_v{latest_version}_docs"),
            latest_version,
            "/",
            &format!("v{latest_version}/docs"),
            &mut defmaker,
        )?);
    }

    // Sort paths
    static_services.sort_by_cached_key(|x| x.path.clone());
    paths.sort_by_cached_key(|x| x.path.clone());

    Ok(RustModule {
        api: ApiService {
            definitions,
            operations,
            paths,
            static_services,
        },
    })
}
