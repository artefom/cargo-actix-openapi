#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

mod generator;
mod openapictx;

use anyhow::Result;

fn main() -> Result<()> {
    generator::generate_api("Hello")?;
    Ok(())
}
