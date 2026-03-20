#[tokio::main]
async fn main() -> eyre::Result<()> {
    // TODO: resolve RPC URL, check with eth_chainId
    // TODO: pull live block (latest or specific number)
    // TODO: pull head and [(tx, call)] from block
    // TODO: execute block with revm and yevm
    // TODO: verify revm result state agains yevm

    // TODO: find a way to stream traces from yevm and revm
    // TODO: zip trace streams and compare them on the fly

    // TODO: run embedded database for state storage (yakvdb?)
    // TODO: (consider: sqlite, leveldb, rocksdb, sled)

    // TODO: for each processed block: generate hermetic env
    // (containing all read storage cells by all transactions)
    // (store it alongsize with block updates to allow reverting)
    // (this allows re-running blocks on-demand without RPC calls)

    println!("please come back later");
    Ok(())
}
