use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;
use tera::{Tera, Value};

static T_API: &str = include_str!("static/api.tera");
static T_ENUM: &str = include_str!("static/enum.tera");
static T_STRUCT: &str = include_str!("static/struct.tera");
static T_DEFAULT: &str = include_str!("static/default.tera");
static T_ERROR: &str = include_str!("static/error.tera");

#[derive(Debug, Serialize)]
pub struct RustEnumVariant {
    pub title: String,
    pub annotation: Option<String>,
    pub data: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RustEnum {
    pub doc: Option<String>,
    pub title: String,
    pub variants: Vec<RustEnumVariant>,
    pub tag: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RustProp {
    pub title: String,
    pub doc: Option<String>,
    pub annotation: Option<String>,
    pub type_: String,
}

#[derive(Debug, Serialize)]
pub struct RustStruct {
    pub doc: Option<String>,
    pub title: String,
    pub props: Vec<RustProp>,
}

#[derive(Debug, Serialize)]
pub struct RustDefault {
    pub title: String,
    pub type_: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct RustErrorVariant {
    pub title: String,
    pub status: String,
    pub display: String,
}

#[derive(Debug, Serialize)]
pub struct RustError {
    pub doc: Option<String>,
    pub title: String,
    pub variants: Vec<RustErrorVariant>,
}

#[derive(Debug, Serialize)]
pub struct RustMethodArg {
    pub name: String,
    pub type_: String,
}

#[derive(Debug, Serialize)]
pub struct RustMethod {
    pub operation_id: String,
    pub doc: Option<String>,
    pub response_type: String,
    pub args: Vec<RustMethodArg>,
}

#[derive(Debug, Serialize)]
pub struct MethodPath {
    pub operation_id: String,
    pub path: String,
    pub method: String,
}

#[derive(Debug, Serialize)]
pub struct StaticInclude {
    pub title: String,
    pub file_path: String,
}

#[derive(Debug, Serialize)]
pub struct StaticString {
    pub title: String,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct StaticHtml {
    pub title: String,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct StaticRedirect {
    pub title: String,
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct StaticService {
    pub method: String,
    pub path: String,
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct RustModule {
    pub enums: Vec<RustEnum>,
    pub structs: Vec<RustStruct>,
    pub defaults: Vec<RustDefault>,
    pub errors: Vec<RustError>,
    pub methods: Vec<RustMethod>,
    pub paths: Vec<MethodPath>,
    pub redirects: Vec<StaticRedirect>,
    pub static_includes: Vec<StaticInclude>,
    pub static_strings: Vec<StaticString>,
    pub static_htmls: Vec<StaticHtml>,
    pub static_services: Vec<StaticService>,
}

pub fn quote_str(value: &str) -> String {
    format!(
        r#""{}""#,
        value.replace('\\', r#"\\"#).replace('"', r#"\""#),
    )
}

fn quote(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::String(value) => value,
        _ => return Err(tera::Error::msg("Can only quote strings")),
    };
    Ok(Value::String(quote_str(value)))
}

fn newline(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::Null => return Ok(Value::Null),
        Value::String(value) => value,
        _ => return Err(tera::Error::msg("Can add new line to strings")),
    };
    Ok(Value::String(format!("\n{value}")))
}

fn comment(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::Null => return Ok(Value::Null),
        Value::String(value) => value,
        _ => return Err(tera::Error::msg("Maybe can only accept string or null")),
    };

    let mut lines = Vec::new();

    for line in value.lines() {
        if line.trim().is_empty() {
            lines.push("///".to_string());
        } else {
            lines.push(format!("/// {}", line));
        }
    }
    Ok(Value::String(lines.join("\n")))
}

fn indent(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::Null => return Ok(Value::Null),
        Value::String(value) => value,
        _ => return Err(tera::Error::msg("Maybe can only accept string or null")),
    };

    let Some(Value::Number(indent_num)) = args.get("n") else {
        return Err(tera::Error::msg("'n' not provided"));
    };

    let indent_num = match indent_num.as_i64() {
        Some(value) => value,
        None => return Err(tera::Error::msg("'n' not i64")),
    };

    let indent_num = match TryInto::<usize>::try_into(indent_num) {
        Ok(value) => value,
        Err(_) => return Err(tera::Error::msg("'n' is not usize")),
    };

    let indent = " ".repeat(indent_num);

    let mut indented_lines = Vec::new();

    for line in value.lines() {
        if line.trim().is_empty() {
            indented_lines.push("".to_string());
        } else {
            indented_lines.push(format!("{indent}{line}"));
        }
    }

    let result = indented_lines.join("\n");

    Ok(Value::String(result))
}

pub fn render_rust_module(module: RustModule) -> Result<String> {
    let mut tera = Tera::default();

    tera.register_filter("quote", quote);
    tera.register_filter("comment", comment);
    tera.register_filter("indent", indent);
    tera.register_filter("newline", newline);

    tera.add_raw_template("enum.tera", T_ENUM)?;
    tera.add_raw_template("error.tera", T_ERROR)?;
    tera.add_raw_template("struct.tera", T_STRUCT)?;
    tera.add_raw_template("default.tera", T_DEFAULT)?;
    tera.add_raw_template("api.tera", T_API)?;

    let ctx = tera::Context::from_serialize(module)?;

    Ok(tera.render("api.tera", &ctx)?)
}
