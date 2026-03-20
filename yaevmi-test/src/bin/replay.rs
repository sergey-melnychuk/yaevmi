use yaevmi_core::{cache::Cache, chain::Chain, exe::Executor, rpc::Rpc};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();
    let url = std::env::var("YAEVMI_RPC_URL").unwrap();
    let rpc = Rpc::latest(url).await?;
    println!("Chain ID: {}", rpc.chain_id().await?);
    println!("Block Hash: {}", rpc.block_hash);
    println!("Block Number: {}", rpc.block_number);

    let mut cache = Cache::new();

    let (call, tx, head) = {
        let block = rpc.block(rpc.block_number).await?;
        let tx = &block.txs[0];
        (tx.call.clone().into(), tx.tx.clone(), block.head)
    };
    println!("Tx Hash: {}", tx.hash);
    println!("Tx Index: {}", tx.index.as_u64());

    let mut exe = Executor::new(call);
    let res = exe.run(tx, head, &mut cache, &rpc).await?;

    println!("Result: {:#?}", res);

    // TODO: execute block with revm and yevm
    // TODO: verify revm result state agains yevm

    // TODO: find a way to stream traces from yevm and revm
    // TODO: zip trace streams and compare them on the fly

    Ok(())
}

// TODO: run embedded database for state storage (yakvdb?)
// TODO: (consider: sqlite, leveldb, rocksdb, sled)

// TODO: for each processed block: generate hermetic env
// (containing all read storage cells by all transactions)
// (store it alongsize with block updates to allow reverting)
// (this allows re-running blocks on-demand without RPC calls)
