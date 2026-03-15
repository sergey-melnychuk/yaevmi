use std::{collections::HashMap, fs::File, io::BufReader};

use serde::Deserialize;
use yaevmi_base::{Int, math::lift};
use yaevmi_core::{
    Call, Head, Tx,
    cache::{Cache, Env},
    exe::{CallResult, Executor},
    state::Account,
    trace::{Event, Step},
};
use yaevmi_misc::buf::Buf;

use crate::eth::EmptyChain;

pub mod auths;
pub mod count;

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

pub async fn run(
    call: Call,
    head: Head,
    env: Env,
    tx: Tx,
) -> eyre::Result<(Int, Buf, i64, Vec<Step>, Env)> {
    let mut state = Cache::new();
    state.insert_account(head.coinbase, Account::default());
    for (acc, info, storage) in env {
        state.insert_account(acc, info);
        for (key, val) in storage {
            state.insert_storage(acc, key, val);
        }
    }

    let mut exe = Executor::new(call);
    let chain = EmptyChain;
    let res = exe.run(tx, head, &mut state, &chain).await?;

    let steps = state
        .events
        .iter()
        .filter_map(|trace| match &trace.event {
            Event::Step(step) => Some(step.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();

    let snapshot = state.snapshot();

    Ok(match res {
        CallResult::Created { acc: addr, code, gas } => (addr.to(), code, gas.spent, steps, snapshot),
        CallResult::Done { status, ret, gas } => (status, ret, gas.spent, steps, snapshot),
    })
}

pub fn ethers(eth: i32) -> Int {
    let exp = lift(|[eth, a, b]| eth * a.pow(b));
    exp([Int::from(eth), Int::from(10), Int::from(18)])
}

#[test]
fn test_load_combined_json() {
    let combined = load().unwrap();
    assert!(!combined.contracts.is_empty());
    assert!(combined.contracts.contains_key("sol/hello.sol:Hello"));
}
