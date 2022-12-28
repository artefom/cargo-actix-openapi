use anyhow::Result;
use serde::Serialize;
use tera::Tera;

static T_API: &'static str = include_str!("static/api.tera");
static T_ENUM: &'static str = include_str!("static/enum.tera");

#[derive(Serialize)]
pub struct RustEnum {
    pub title: String,
    pub variants: Vec<String>,
}

#[derive(Serialize)]
pub struct RustModule {
    pub enums: Vec<RustEnum>,
}

pub fn render_rust_module(module: RustModule) -> Result<String> {
    let mut tera = Tera::default();

    tera.add_raw_template("enum.tera", T_ENUM)?;
    tera.add_raw_template("api.tera", T_API)?;

    let ctx = tera::Context::from_serialize(&module)?;

    Ok(tera.render("api.tera", &ctx)?)
}
