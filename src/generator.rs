use indexmap::IndexMap;

use anyhow::{Context, Result};

mod models;
mod templates;

use models::to_rust_module;

use self::models::{
    types::{
        DefaultProvider, OperationPath, RApiErr, REnum, RStruct, RustOperation, StaticHtmlPath,
        StaticRedirect, StaticStr, StaticStringPath,
    },
    OpenApiWithPath,
};

fn convert_enums(name: &str, enum_def: &REnum) -> templates::RustEnum {
    let mut variants = Vec::new();

    for variant in &enum_def.variants {
        let mut annotation = IndexMap::new();

        if variant.rename != variant.name {
            annotation.insert("rename", variant.rename.clone());
        }

        variants.push(templates::RustEnumVariant {
            title: variant.name.clone(),
            annotation: render_annotation(annotation),
            data: variant.data.as_ref().map(|x| x.to_string()),
        })
    }

    templates::RustEnum {
        doc: enum_def.doc.clone(),
        title: name.to_string(),
        variants,
        tag: enum_def.discriminator.clone(),
    }
}

fn render_annotation(vals: IndexMap<&str, String>) -> Option<String> {
    let mut keyvals: Vec<String> = Vec::new();

    for (key, value) in vals {
        let value = templates::quote_str(&value);
        keyvals.push(format!("{key} = {value}"))
    }

    if keyvals.is_empty() {
        return None;
    }

    let keyvals = keyvals.join(", ");

    Some(format!("#[serde({keyvals})]"))
}

fn convert_struct(name: &str, struct_def: &RStruct) -> templates::RustStruct {
    let mut props = Vec::new();

    for prop in &struct_def.properties {
        let mut annotation = IndexMap::new();

        if prop.rename != prop.name {
            annotation.insert("rename", prop.rename.clone());
        };

        if let Some(ref default) = prop.default {
            annotation.insert("default", default.to_string());
        };

        props.push(templates::RustProp {
            title: prop.name.clone(),
            doc: prop.doc.clone(),
            annotation: render_annotation(annotation),
            type_: prop.type_.to_string(),
        })
    }

    templates::RustStruct {
        doc: struct_def.doc.clone(),
        title: name.to_string(),
        props,
    }
}

fn convert_defaults(name: &str, default: &DefaultProvider) -> templates::RustDefault {
    templates::RustDefault {
        title: name.to_string(),
        type_: default.vtype.to_string(),
        value: default.value.to_string(),
    }
}

fn convert_error(name: &str, err: &RApiErr) -> templates::RustError {
    let mut variants = Vec::new();

    for variant in &err.variants {
        variants.push(templates::RustErrorVariant {
            title: variant.name.clone(),
            status: variant.code.clone(),
            display: variant.detail.clone(),
        })
    }

    templates::RustError {
        doc: err.doc.clone(),
        title: name.to_string(),
        variants,
    }
}

fn convert_method(name: &str, op: &RustOperation) -> templates::RustMethod {
    let mut args = Vec::new();

    if let Some(param) = &op.param_path {
        args.push(templates::RustMethodArg {
            name: "path".to_string(),
            type_: param.to_string(),
        })
    }

    if let Some(param) = &op.param_query {
        args.push(templates::RustMethodArg {
            name: "query".to_string(),
            type_: param.to_string(),
        })
    }

    if let Some(param) = &op.param_body {
        args.push(templates::RustMethodArg {
            name: "body".to_string(),
            type_: param.to_string(),
        })
    }

    templates::RustMethod {
        operation_id: name.to_string(),
        response_type: op.response.to_string(),
        doc: op.doc.clone(),
        args,
    }
}

fn convert_method_path(path: &OperationPath) -> templates::MethodPath {
    templates::MethodPath {
        operation_id: path.operation.clone(),
        path: path.path.clone(),
        method: path.method.to_string(),
    }
}

fn convert_include(name: &str, value: &StaticStr) -> templates::StaticInclude {
    templates::StaticInclude {
        title: name.to_string(),
        file_path: value.path.clone(),
    }
}

fn convert_static_string(name: &str, value: &StaticStringPath) -> templates::StaticString {
    templates::StaticString {
        title: name.to_string(),
        data: value.data.clone(),
    }
}

fn convert_static_html(name: &str, value: &StaticHtmlPath) -> templates::StaticHtml {
    templates::StaticHtml {
        title: name.to_string(),
        data: value.data.clone(),
    }
}

fn convert_redirect(name: &str, value: &StaticRedirect) -> templates::StaticRedirect {
    templates::StaticRedirect {
        title: name.to_string(),
        target: value.target.clone(),
    }
}

pub struct OpenapiWithMeta {
    pub content: String,
    pub path: String,
}

pub fn generate_api(docs_path: &str, specs: &[OpenapiWithMeta]) -> Result<(String, String)> {
    let mut openapis: Vec<OpenApiWithPath> = Vec::new();

    for spec in specs {
        let content = serde_yaml::from_str(&spec.content).context("Could not deserialize input")?;
        openapis.push(OpenApiWithPath {
            spec_path: spec.path.to_string(),
            spec: content,
        });
    }

    let rust_module =
        to_rust_module(docs_path, &openapis).context("Could not generate rust module")?;

    let serialized_model = serde_yaml::to_string(&rust_module)?;

    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut errors = Vec::new();
    let mut defaults = Vec::new();
    let mut static_includes = Vec::new();
    let mut static_strings = Vec::new();
    let mut static_htmls = Vec::new();
    let mut redirects = Vec::new();

    for (def_name, def) in &rust_module.api.definitions {
        {
            use models::types::DefinitionData::*;
            match &def.data {
                Struct(value) => structs.push(convert_struct(def_name, value)),
                Enum(value) => enums.push(convert_enums(def_name, value)),
                ApiErr(value) => errors.push(convert_error(def_name, value)),
                DefaultProvider(value) => defaults.push(convert_defaults(def_name, value)),
                StaticStr(value) => static_includes.push(convert_include(def_name, value)),
                StaticStringPath(value) => {
                    static_strings.push(convert_static_string(def_name, value))
                }
                StaticHtmlPath(value) => static_htmls.push(convert_static_html(def_name, value)),
                Redirect(value) => redirects.push(convert_redirect(def_name, value)),
            }
        }
    }

    let mut methods = Vec::new();

    for (method_name, method) in &rust_module.api.operations {
        methods.push(convert_method(method_name, method));
    }

    let mut paths = Vec::new();

    for path in &rust_module.api.paths {
        paths.push(convert_method_path(path))
    }

    let mut static_services = Vec::new();

    for service in &rust_module.api.static_services {
        static_services.push(templates::StaticService {
            method: service.method.to_string(),
            path: service.path.clone(),
            target: service.data.clone(),
        })
    }

    let rust_module = templates::RustModule {
        structs,
        enums,
        defaults,
        errors,
        methods,
        paths,
        static_includes,
        static_strings,
        static_htmls,
        static_services,
        redirects,
    };

    let serialized = templates::render_rust_module(rust_module)?;

    Ok((serialized_model, serialized))
}
