use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;
use tera::{Tera, Value};

static T_API: &str = include_str!("static/api.tera");
static T_ENUM: &str = include_str!("static/enum.tera");

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
pub struct RustModule {
    pub enums: Vec<RustEnum>,
}

fn quote(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::String(value) => value,
        _ => return Err(tera::Error::msg("Can only quote strings")),
    };
    let value = format!(
        r#""{}""#,
        value.replace('\\', r#"\\"#).replace('"', r#"\""#),
    );
    Ok(Value::String(value))
}

fn comment(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let value = match value {
        Value::Null => return Ok(Value::String("".to_string())),
        Value::String(value) => value,
        _ => return Err(tera::Error::msg("Maybe can only accept string or null")),
    };

    let mut lines = Vec::new();

    for line in value.split('\n') {
        lines.push(format!("/// {}", line));
    }

    Ok(Value::String(lines.join("\n")))
}

pub fn render_rust_module(module: RustModule) -> Result<String> {
    let mut tera = Tera::default();

    tera.register_filter("quote", quote);
    tera.register_filter("comment", comment);

    tera.add_raw_template("enum.tera", T_ENUM)?;
    tera.add_raw_template("api.tera", T_API)?;

    let ctx = tera::Context::from_serialize(module)?;

    Ok(tera.render("api.tera", &ctx)?)
}
