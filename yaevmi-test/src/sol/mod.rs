// TODO: Add tests against local solidity contracts

use std::{collections::HashMap, fs::File, io::BufReader};

use serde::Deserialize;
use yaevmi_base::{Acc, Int, acc, int, math::lift};
use yaevmi_core::{
    Call, Head,
    cache::Cache,
    exe::{CallResult, Executor},
    state::Account,
    trace::{Event, Step},
};
use yaevmi_misc::buf::Buf;

use crate::eth::EmptyChain;

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
    env: Vec<(Acc, Account, Vec<(Int, Int)>)>,
) -> eyre::Result<(Int, Buf, i64, Vec<Step>)> {
    let mut state = Cache::new();
    for (acc, info, storage) in env {
        state.insert_account(acc, info);
        for (key, val) in storage {
            state.insert_storage(acc, key, val);
        }
    }

    let mut exe = Executor::new(call);
    let mut chain = EmptyChain;
    let res = exe.run(head, &mut state, &mut chain).await?;

    let steps = state
        .events
        .iter()
        .filter_map(|trace| match &trace.event {
            Event::Step(step) => Some(step.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();

    Ok(match res {
        CallResult::Created { addr, code, gas } => (addr.to(), code, gas.spent, steps),
        CallResult::Done { status, ret, gas } => (status, ret, gas.spent, steps),
    })
}

pub fn ethers(eth: i32) -> Int {
    let exp = lift(|[eth, a, b]| eth * a.pow(b));
    exp([Int::from(eth), Int::from(10), Int::from(18)])
}

#[test]
fn test_load() {
    let combined = load().unwrap();
    assert!(!combined.contracts.is_empty());
    assert!(combined.contracts.contains_key("sol/hello.sol:Hello"));
}

#[tokio::test]
async fn test_deploy_counter() -> eyre::Result<()> {
    let combined = load()?;
    let contract = &combined.contracts["sol/count.sol:Count"];

    let sender = acc("0xAA");
    let nonce = Int::ZERO;

    let env = vec![(
        sender,
        Account {
            value: ethers(1),
            nonce,
            code: (Buf::default(), Int::ZERO),
        },
        vec![(int("0xFF"), int("0xFF"))],
    )];
    let head = Head {
        number: 1,
        hash: int("0x1"),
        gas_price: 1.into(),
        gas_limit: 1_000_000.into(),
        coinbase: acc("0xC014BA5E"),
        timestamp: 42.into(),
        base_fee: 1.into(),
        blob_base_fee: 1.into(),
        chain_id: 1,
        blobhash: Int::ONE,
        prevrandao: Int::ONE,
    };
    let call = Call {
        by: sender,
        to: Acc::ZERO,
        gas: 1_000_000,
        eth: Int::ZERO,
        data: contract.bin.clone(),
        auth: vec![],
        nonce: None,
    };

    // for line in text(call.data.as_slice()) {
    //     println!("{line}");
    // }

    let exp = crate::revm::run(call.clone(), head.clone(), env.clone()).await?;
    let res = run(call, head, env).await?;

    pretty_assertions::assert_eq!(res, exp);

    /*
    let (acc, ret, gas) = run(call, head, env).await?;
    assert!(gas >= 0, "gas charged"); // TODO: update after gas finalisation impl

    let created = create_address(&sender, nonce.as_u64()).to::<32>();
    assert_eq!(acc, created, "acc created");

    assert_eq!(ret.as_slice(), contract.bin_runtime.as_slice());
     */
    Ok(())
}
