#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

mod generator;
mod openapictx;

use std::{
    env,
    fs::{read_dir, read_to_string},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::Serialize;
use tera::Tera;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the source openapi file
    spec_dir: PathBuf,
    out_path: PathBuf,
}

fn is_openapi_spec(path: &Path) -> bool {
    let Some(extension) = path.extension() else {
        return false;
    };
    let Some(stem) = path.file_stem() else {
        return false;
    };
    return extension == "yaml" && stem.to_string_lossy().contains("openapi");
}

fn is_doc_page(path: &Path) -> bool {
    let Some(filename) = path.file_name() else {
        return false;
    };
    filename.to_ascii_lowercase() == "docs.html"
}

fn prompt_user(message: &str) -> Result<bool> {
    loop {
        print!("{} (Y/n): ", message);

        std::io::stdout()
            .flush()
            .context("Could not flush stdout")?;

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Could not get user input")?;
        input = input.trim().to_lowercase();
        match input.as_str() {
            "y" => return Ok(true),
            "n" => return Ok(false),
            "" => return Ok(true),
            _ => continue,
        }
    }
}

fn get_user_input(message: &str, default: &str) -> Result<String> {
    print!("{} [{}]: ", message, default);

    std::io::stdout()
        .flush()
        .context("Could not flush stdout")?;

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context("Could not get user input")?;
    input = input.trim().to_lowercase();
    match input.as_str() {
        "" => Ok(default.to_string()),
        value => Ok(value.to_string()),
    }
}

#[derive(Serialize)]
struct DocsRenderContext {
    title: String,
}

static DOCS_HTML: &str = include_str!("generator/static/docs.html");

fn create_docs_file(target_dir: &Path, title: &str) -> Result<PathBuf> {
    let mut target_file = target_dir.to_path_buf();
    target_file.push("docs.html");

    let mut tera = Tera::default();

    let ctx = tera::Context::from_serialize(DocsRenderContext {
        title: title.to_string(),
    })?;

    tera.add_raw_template("docs.html", DOCS_HTML)?;

    let rendered = tera
        .render("docs.html", &ctx)
        .context("Could not render docs.html")?;

    std::fs::write(target_file.clone(), rendered).context("Could not write docs.html")?;

    Ok(target_file)
}

fn scan_dir(target_file: &Path, dir: &Path) -> Result<(String, Vec<generator::OpenapiWithMeta>)> {
    let paths = read_dir(dir)?;
    let mut openapi_files = Vec::new();

    let mut doc_path: Option<PathBuf> = None;

    let mut target_dir = target_file.to_owned();
    target_dir.pop();

    for path in paths {
        let path = path?;
        let path = if path.file_type()?.is_file() {
            path.path()
        } else {
            continue;
        };

        if is_openapi_spec(&path) {
            let Some(path_rel) = pathdiff::diff_paths(&path, &target_dir) else {
                bail!(format!("Could not express path {} relative to {}", path.to_string_lossy(), 
                target_dir.to_string_lossy()))
            };

            let Some(path_rel) = path_rel.to_str() else {
                bail!(format!("Cannot represent relative {} path as string", path_rel.to_string_lossy())) 
            };
            openapi_files.push(generator::OpenapiWithMeta {
                content: read_to_string(&path).context("Could not read file")?,
                path: path_rel.to_string(),
            })
        }
        if is_doc_page(&path) {
            if let Some(doc_path) = doc_path {
                bail!(
                    "Got two doc paths! One is {} second is {}",
                    doc_path.to_string_lossy(),
                    path.to_string_lossy()
                )
            }
            doc_path = Some(path);
        }
    }

    let doc_path = match doc_path {
        Some(value) => value,
        None => {
            let user_agrees = prompt_user("Could not find docs.html. Create?").context(
                "Could not find doc.html, and could not get user permission to create it",
            )?;
            if !user_agrees {
                bail!("Could not find docs.html and User denied creation of it");
            };
            let title = get_user_input("Please, enter application name", "App")
                .context("Could not get application name from user")?;
            create_docs_file(dir, &title)?
        }
    };

    let Some(doc_path) = pathdiff::diff_paths(&doc_path, &target_dir) else {
        bail!(format!("Could not express path {} relative to {}", doc_path.to_string_lossy(), 
        target_dir.to_string_lossy()))
    };

    let Some(doc_path) = doc_path.to_str() else {
        bail!("Could not represend doc path {} as string", doc_path.to_string_lossy())
    };

    Ok((doc_path.to_string(), openapi_files))
}

fn main() -> Result<()> {
    let mut args: Vec<String> = env::args().collect();

    // Fix when running script as cargo actix-openapi
    if let Some(val) = args.get(1) {
        if val == "actix-openapi" {
            args.remove(1);
        }
    }

    let args = Args::parse_from(args);

    let (docs_file, openapi_specs) = scan_dir(&args.out_path, &args.spec_dir)?;

    let (_, generated) = generator::generate_api(&docs_file, &openapi_specs)?;

    std::fs::write(args.out_path.clone(), generated)
        .with_context(|| format!("Could not result into {}", args.out_path.to_string_lossy()))?;

    Ok(())
}
