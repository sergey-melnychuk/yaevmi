// TODO: Add tests against local solidity contracts

use std::{collections::HashMap, fs::File, io::BufReader};

use serde::Deserialize;
use yaevmi_misc::buf::Buf;

#[derive(Deserialize)]
pub struct Combined {
    pub contracts: HashMap<String, Contract>,
}

#[derive(Deserialize)]
pub struct Contract {
    pub bin: Buf,
    #[serde(rename = "bin-runtime")]
    pub bin_runtime: Buf,
}

pub fn load() -> Result<Combined, eyre::Report> {
    let file = File::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/sol/bin/combined.json"
    ))?;
    let reader = BufReader::new(file);
    let combined: Combined = serde_json::from_reader(reader)?;
    Ok(combined)
}

#[test]
fn test_load() {
    let combined = load().unwrap();
    assert!(!combined.contracts.is_empty());
    assert!(combined.contracts.contains_key("sol/hello.sol:Hello"));
}
