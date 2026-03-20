use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use yaevmi_base::{Acc, Int};
use yaevmi_misc::{buf::Buf, http::Http};

use crate::{
    call::{Block, Head},
    chain::Chain,
    state::Account,
};

pub struct Rpc {
    url: String,
    http: Http,
    pub block_number: u64,
    pub block_hash: Int,
}

impl Rpc {
    pub async fn latest(url: String) -> eyre::Result<Self> {
        let http = Http::new();
        let head: Head = latest(&http, &url).await?;
        Ok(Self {
            url,
            http,
            block_number: head.number.as_u64() - 1,
            block_hash: head.parent_hash,
        })
    }

    pub async fn chain_id(&self) -> eyre::Result<u32> {
        let chain_id: Int = call(&self.http, &self.url, "eth_chainId", &[]).await?;
        Ok(chain_id.as_u32())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl Chain for Rpc {
    async fn get(&self, acc: &Acc, key: &Int) -> eyre::Result<Int> {
        let value = call(
            &self.http,
            &self.url,
            "eth_getStorageAt",
            &[
                Value::String(acc.to_string()),
                Value::String(key.to_string()),
                Value::String(self.block_hash.to_string()),
            ],
        )
        .await?;
        Ok(value)
    }

    async fn acc(&self, acc: &Acc) -> eyre::Result<Account> {
        // TODO: consider firing sub-calls concurrently to speed this up
        Ok(Account {
            value: self.balance(acc).await?,
            nonce: self.nonce(acc).await?.into(),
            code: self.code(acc).await?,
        })
    }

    async fn code(&self, acc: &Acc) -> eyre::Result<(Buf, Int)> {
        let code: Buf = call(
            &self.http,
            &self.url,
            "eth_getCode",
            &[
                Value::String(acc.to_string()),
                Value::String(self.block_hash.to_string()),
            ],
        )
        .await?;
        let hash = yaevmi_misc::keccak256(code.as_slice());
        Ok((code, hash))
    }

    async fn nonce(&self, acc: &Acc) -> eyre::Result<u64> {
        let nonce: Int = call(
            &self.http,
            &self.url,
            "eth_getTransactionCount",
            &[
                Value::String(acc.to_string()),
                Value::String(self.block_hash.to_string()),
            ],
        )
        .await?;
        Ok(nonce.as_u64())
    }

    async fn balance(&self, acc: &Acc) -> eyre::Result<Int> {
        let balance = call(
            &self.http,
            &self.url,
            "eth_getBalance",
            &[
                Value::String(acc.to_string()),
                Value::String(self.block_hash.to_string()),
            ],
        )
        .await?;
        Ok(balance)
    }

    async fn head(&self, number: u64) -> eyre::Result<Head> {
        let head = call(
            &self.http,
            &self.url,
            "eth_getBlockByNumber",
            &[Value::String(format!("0x{:x}", number)), Value::Bool(false)],
        )
        .await?;
        Ok(head)
    }

    async fn block(&self, number: u64) -> eyre::Result<Block> {
        let block = call(
            &self.http,
            &self.url,
            "eth_getBlockByNumber",
            &[Value::String(format!("0x{:x}", number)), Value::Bool(true)],
        )
        .await?;
        Ok(block)
    }
}

async fn latest(http: &Http, url: &str) -> eyre::Result<Head> {
    let head = call(
        http,
        url,
        "eth_getBlockByNumber",
        &[Value::String("latest".to_string()), Value::Bool(false)],
    )
    .await?;
    Ok(head)
}

async fn call<R: DeserializeOwned>(
    http: &Http,
    url: &str,
    method: &str,
    params: &[Value],
) -> eyre::Result<R> {
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1,
    });
    let json: Value = http.post(url, &body).await?;
    if std::env::var("DEBUG").is_ok() {
        // Just for debugging until proper logging is implemented
        let body = serde_json::to_string_pretty(&body).unwrap();
        let json = serde_json::to_string_pretty(&json).unwrap();
        println!("{} -> {}", body, json);
    }
    if let Some(error) = json.get("error") {
        if let Some(message) = error.as_str() {
            eyre::bail!(message.to_owned());
        }
        let message = serde_json::to_string(error)?;
        eyre::bail!(message);
    }
    if let Some(result) = json.get("result") {
        if result.is_null() {
            eyre::bail!("result is null");
        } else {
            if let Ok(result) = serde_json::from_value::<R>(result.to_owned()) {
                return Ok(result);
            } else {
                eprintln!("RESULT: {:#?}", result);
            }
        }
    }
    eprint!("JSON: {:#?}", json);
    eyre::bail!("missing: result & error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "makes RPC call to a public node"]
    async fn test_latest() -> eyre::Result<()> {
        let http = Http::new();
        let head = latest(&http, "https://ethereum-rpc.publicnode.com").await?;
        let (number, hash) = (head.number.as_u64(), head.hash);
        assert_ne!(hash, Int::ZERO);
        assert!(number > 24697386);
        Ok(())
    }
}
