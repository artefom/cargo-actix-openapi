use std::{
    fs::{read_to_string, File},
    path::PathBuf,
};

use std::io::prelude::Write;

#[derive(Debug)]
pub struct TestCase {
    pub spec: String,
    pub expected: Option<String>,
}

fn get_root_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path
}

fn get_spec_path(name: &str) -> PathBuf {
    let mut path = get_root_path();
    path.push("openapi");
    path.push(format!("{name}.yaml"));
    path
}

fn get_expected_path(name: &str) -> PathBuf {
    let mut path = get_root_path();
    path.push("expected");
    path.push(format!("{name}.rs"));
    path
}

pub fn load_openapi_case(name: &str) -> Result<TestCase, std::io::Error> {
    let spec_path = get_spec_path(name);
    let expected_path = get_expected_path(name);

    let expected = read_to_string(expected_path).ok();

    Ok(TestCase {
        spec: read_to_string(spec_path)?,
        expected,
    })
}

pub fn save_expected(name: &str, expected: &String) -> Result<(), std::io::Error> {
    let expected_path = get_expected_path(name);

    let mut file = File::create(expected_path)?;
    file.write_all(expected.as_bytes())?;

    Ok(())
}
