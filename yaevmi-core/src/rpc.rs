#![allow(dead_code)] // TODO: remove this

use serde_json::{Value, json};
use yaevmi_base::{
    Acc, Int,
    dto::{Head, Tx},
};
use yaevmi_misc::http::Http;

use crate::state::{Account, Chain};

pub struct Rpc {
    url: String,
    http: Http,
    // hash: Int,
}

impl Rpc {
    pub fn new(url: String) -> Self {
        Self {
            url,
            http: Http::new(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl Chain for Rpc {
    async fn get(&self, _acc: &Acc, _key: &Int) -> eyre::Result<Int> {
        eyre::bail!("WIP")
    }

    async fn acc(&self, _acc: &Acc) -> eyre::Result<Account> {
        eyre::bail!("WIP")
    }

    async fn code(&self, _acc: &Acc) -> eyre::Result<(Vec<u8>, Int)> {
        eyre::bail!("WIP")
    }

    async fn nonce(&self, _acc: &Acc) -> eyre::Result<u64> {
        eyre::bail!("WIP")
    }

    async fn balance(&self, _acc: &Acc) -> eyre::Result<Int> {
        eyre::bail!("WIP")
    }

    async fn head(&self, _number: u64) -> eyre::Result<Head> {
        eyre::bail!("WIP")
    }

    async fn block(&self, _number: u64) -> eyre::Result<(Head, Vec<Tx>)> {
        eyre::bail!("WIP")
    }
}

async fn latest(http: &Http, url: &str) -> eyre::Result<(u64, Int)> {
    let json = call(
        http,
        url,
        "eth_getBlockByNumber",
        &[Value::String("latest".to_string()), Value::Bool(false)],
    )
    .await?;
    eprintln!("{}", serde_json::to_string_pretty(&json).unwrap());
    eyre::bail!("WIP")
    // TODO: parse Head from JSON and return block number & hash
}

async fn call(http: &Http, url: &str, method: &str, params: &[Value]) -> eyre::Result<Value> {
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1,
    });
    let json: Value = http.post(url, &body).await?;
    if let Some(result) = json.get("result") {
        return Ok(result.to_owned());
    }
    if let Some(error) = json.get("error") {
        if let Some(message) = error.as_str() {
            eyre::bail!(message.to_owned());
        }
        let message = serde_json::to_string(error)?;
        eyre::bail!(message);
    }
    eyre::bail!("missing: result & error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_latest() -> eyre::Result<()> {
        let http = Http::new();
        let _ = latest(&http, "https://ethereum-rpc.publicnode.com").await?;
        // TODO: make hermetic
        Ok(())
    }
}

/*

{
  "baseFeePerGas": "0x246edf3",
  "blobGasUsed": "0x40000",
  "difficulty": "0x0",
  "excessBlobGas": "0xa2a720e",
  "extraData": "0x4275696c6465724e6574202842656176657229",
  "gasLimit": "0x3938700",
  "gasUsed": "0x1a4ed69",
  "hash": "0x6c4c6ea047f0ca4bcb0b0fbbcc2e50061d8dd24ba1ab8a22a6d3583a497c3c82",
  "logsBloom": "0xfffffef97ffdf7dfb75fbbfeffffdfffffbeefffffbbffff6ffffdffbffffaf9df7fff7f7fff6ffbffffdfdffbffffff76fffffcbf7ffbfffffeefffffff7df7bffffffdefffff8fdfdf7f7ceffdffff7ff77affe7ef5ff5fbfff57ff7eeadfffffffabfff7bdffbbfbffcfffdefffeffefffffffff5fffefdffdffbffffdfdffbffe7fffffffffdffebf7e37fdfe977f7fbdfefdffddffffffffff5f97ffffdfbffdf7fffffbff9feff77afffefffeef77ffff7fff6fffeffd77ffffeefefdb7bffffff7f7ffdfbff7ffffffffefdffdffddffffffefffffffefdfbf7fdffef7fffff3bdf7fff6cefffdfffdf7ffffeffbfbffffdafffdfbf7efdbeffff7fff",
  "miner": "0xdadb0d80178819f2319190d340ce9a924f783711",
  "mixHash": "0xf824cbc0bffaa4862fffafa96c2cce90898dda8b97fc3eaa6a85ac38075991e5",
  "nonce": "0x0000000000000000",
  "number": "0x177d354",
  "parentBeaconBlockRoot": "0xd9b957562f8477151f61eddc77bcad7451ef62eab8f4fe470402073a794e8bba",
  "parentHash": "0x63ce16602b2afe4089fa8e0a2c16be1a2dea1c4d30b65bd717d9398ddd5fba23",
  "receiptsRoot": "0x52fc280ab6b1db61781dae4cc5f81b7ddd484eae78320bdd8161d5b2fe8e7719",
  "requestsHash": "0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
  "size": "0x2a8a9",
  "stateRoot": "0x41ebb402f36d0ce15799e98d1aca83bbb5d4b08d1b6572e73b388e4ece192916",
  "timestamp": "0x69b09d6b",
  "transactions": [
    "0x0a57a0922b5abd25c79e49bbc1bf478ec923f8c580834fc0fd450ab6ad8b9630"
  ],
  "transactionsRoot": "0x3b889eb9fa0713d2e1d9e90725d45fd04ec4a550a7a390639c243c5f3b2c6215",
  "uncles": [],
  "withdrawals": [
    {
      "address": "0xb9d7934878b5fb9610b3fe8a5e441e8fad7e293f",
      "amount": "0x344741",
      "index": "0x73e482b",
      "validatorIndex": "0x21e64c"
    }
  ],
  "withdrawalsRoot": "0x76abcbef0846a5bd065d0207581dacc3cf8df4ec2ba0a8e4d4fbbe97864b82ee"
}

*/
