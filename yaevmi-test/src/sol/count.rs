use yaevmi_base::{Acc, Int, acc, int};
use yaevmi_core::{Call, Head, Tx, state::Account};
use yaevmi_misc::buf::Buf;

use crate::sol::{ethers, load, run};

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
        vec![],
    )];

    let head = Head {
        number: 1.into(),
        hash: int("0x1"),
        gas_limit: 1_000_000.into(),
        coinbase: acc("0xC014BA5E"),
        timestamp: 42.into(),
        base_fee: 1.into(),
        blob_base_fee: Some(1.into()),
        blobhash: Some(Int::ONE),
        prevrandao: Int::ONE,
        parent_hash: int("0x1"),
    };
    let call = Call {
        by: sender,
        to: Acc::ZERO,
        gas: 1_000_000,
        eth: Int::ZERO,
        data: contract.bin.clone(),
    };
    let tx = Tx {
        chain_id: 1.into(),
        nonce: 0.into(),
        gas_price: 1.into(),
        max_fee_per_gas: 1.into(),
        max_priority_fee_per_gas: 1.into(),
        access_list: vec![],
        authorization_list: vec![],
        blob_hashes: vec![],
        max_fee_per_blob_gas: Some(1.into()),
        hash: Int::ZERO,
        index: Int::ZERO,
    };

    let exp = crate::revm::run(call.clone(), head.clone(), env.clone(), tx.clone()).await?;
    let res = run(call, head, env, tx).await?;
    pretty_assertions::assert_eq!(res, exp);
    Ok(())
}
