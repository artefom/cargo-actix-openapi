use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;
use tera::{Tera, Value};

static T_API: &str = include_str!("static/api.tera");
static T_ENUM: &str = include_str!("static/enum.tera");
static T_STRUCT: &str = include_str!("static/struct.tera");

#[derive(Debug, Serialize)]
pub struct RustEnumVariant {
    pub title: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct RustEnum {
    pub doc: Option<String>,
    pub title: String,
    pub variants: Vec<RustEnumVariant>,
}

#[derive(Debug, Serialize)]
pub struct RustProp {
    pub title: String,
    pub annotation: Option<String>,
    pub ptype: String,
}

#[derive(Debug, Serialize)]
pub struct RustStruct {
    pub doc: Option<String>,
    pub title: String,
    pub props: Vec<RustProp>,
}

#[derive(Debug, Serialize)]
pub struct RustModule {
    pub enums: Vec<RustEnum>,
    pub structs: Vec<RustStruct>,
}

pub fn quote_str(value: &str) -> String {
    format!(
        r#""{}""#,
        value.replace('\\', r#"\\"#).replace('"', r#"\""#),
    )
}

fn quote(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::String(value) => value,
        _ => return Err(tera::Error::msg("Can only quote strings")),
    };
    Ok(Value::String(quote_str(value)))
}

fn comment(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::Null => return Ok(Value::String("".to_string())),
        Value::String(value) => value,
        _ => return Err(tera::Error::msg("Maybe can only accept string or null")),
    };

    let mut lines = Vec::new();

    for line in value.trim_end().split('\n') {
        lines.push(format!("/// {}", line));
    }

    Ok(Value::String(format!("{}\n", lines.join("\n"))))
}

fn indent(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::Null => return Ok(Value::String("".to_string())),
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

    Ok(Value::String(format!("\n{indent}{value}")))
}

pub fn render_rust_module(module: RustModule) -> Result<String> {
    let mut tera = Tera::default();

    tera.register_filter("quote", quote);
    tera.register_filter("comment", comment);
    tera.register_filter("indent", indent);

    tera.add_raw_template("enum.tera", T_ENUM)?;
    tera.add_raw_template("struct.tera", T_STRUCT)?;
    tera.add_raw_template("api.tera", T_API)?;

    let ctx = tera::Context::from_serialize(module)?;

    Ok(tera.render("api.tera", &ctx)?)
}
