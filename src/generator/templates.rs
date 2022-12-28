use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;
use tera::{Tera, Value};

static T_API: &'static str = include_str!("static/api.tera");
static T_ENUM: &'static str = include_str!("static/enum.tera");

#[derive(Debug, Serialize)]
pub struct RustEnumVariant {
    pub title: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct RustEnum {
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
        value.replace(r#"\"#, r#"\\"#).replace(r#"""#, r#"\""#),
    );
    Ok(Value::String(value.clone()))
}

pub fn render_rust_module(module: RustModule) -> Result<String> {
    let mut tera = Tera::default();

    tera.register_filter("quote", quote);

    tera.add_raw_template("enum.tera", T_ENUM)?;
    tera.add_raw_template("api.tera", T_API)?;

    let ctx = tera::Context::from_serialize(&module)?;

    Ok(tera.render("api.tera", &ctx)?)
}
