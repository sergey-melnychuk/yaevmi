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
