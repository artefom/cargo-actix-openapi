#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

mod generator;
mod openapictx;

use anyhow::Result;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Serialize,Deserialize, Clone, PartialEq, Eq)]
pub enum EnumTest {
    #[serde(rename = "Hello, world")]
    I1,
    I2,
    I3,
}



#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Test {
    pub a: EnumTest,
    pub b: EnumTest,
}

fn main() -> Result<()> {
    generator::generate_api("Hello")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exploration() -> Result<()> {
        let data = EnumTest::I1;

        let wrapper = Test {
            a: data.clone(),
            b: data.clone(),
        };

        println!("{}", serde_yaml::to_string(&wrapper)?);

        Ok(())
    }
}
