#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

mod generator;
mod openapictx;

use std::{env, fs::read_to_string};

use anyhow::{bail, Context, Result};
use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the source openapi file
    path: String,
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

    if args.path.is_empty() {
        bail!("Openapi path not provided (use --help to see usage)")
    }

    let file_contents = read_to_string(&args.path)
        .with_context(|| format!("Could not open openapi spec at {}", &args.path))?;

    let (_, generated) = generator::generate_api(&file_contents)?;
    println!("{}", generated);
    Ok(())
}
