use anyhow::Result;
use serde::Serialize;
use tera::Tera;

static API_TEMPLATE: &'static str = include_str!("static/api.tera");

#[derive(Serialize)]
struct RustModel {}

#[derive(Serialize)]
struct RustError {}

#[derive(Serialize)]
struct RustProvider {}

#[derive(Serialize)]
struct RustMethod {}

#[derive(Serialize)]
pub struct RustModule {
    models: Vec<RustModel>,
    errors: Vec<RustError>,
    providers: Vec<RustProvider>,
    methods: Vec<RustMethod>,
}

impl Default for RustModule {
    fn default() -> Self {
        Self {
            models: Vec::new(),
            errors: Vec::new(),
            providers: Vec::new(),
            methods: Vec::new(),
        }
    }
}

pub fn render_rust_module(module: RustModule) -> Result<String> {
    let mut tera = Tera::default();

    tera.add_raw_template("api.rs", API_TEMPLATE)?;

    let ctx = tera::Context::from_serialize(&module)?;

    Ok(tera.render("api.rs", &ctx)?)
}
