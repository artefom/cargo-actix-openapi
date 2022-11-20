mod generator;

use anyhow::Result;

fn main() -> Result<()> {
    generator::generate_api("Hello")?;
    Ok(())
}
