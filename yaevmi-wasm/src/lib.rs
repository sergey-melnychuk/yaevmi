// #[cfg(target_arch = "wasm32")] // comment-out this line for development
mod wasm {
    use js_sys::JsString;
    use wasm_bindgen::prelude::*;
    use futures::StreamExt;
    use futures::channel::mpsc;
    use yaevmi_core::Int;
    use yaevmi_core::cache::Cache;
    use yaevmi_core::chain::Chain;
    use yaevmi_core::exe::Executor;
    use yaevmi_core::trace::Step;
    use yaevmi_core::{Call, Head, Tx, rpc::Rpc};

    #[wasm_bindgen]
    pub fn hello(name: JsString) -> JsString {
        JsString::from(format!("Hello, {name}").as_str())
    }

    #[wasm_bindgen]
    pub fn call(json: JsValue) -> Result<JsValue, JsError> {
        let call: Call = serde_wasm_bindgen::from_value(json)?;
        Ok(serde_wasm_bindgen::to_value(&call)?)
    }

    // TODO: YAVMI: Executor, Call & Builder, Int/Acc/Buf, Tx/Head
    /*
    ```javascript
    let call = {
        by: '0x1',
        to: '0x2',
        eth: '0x3',
        gas: 4,
        data: '0x5'
    };
    let exe = new Executor(call);

    let tx = {
        // ...
    };
    let head = {
        // ...
    };
    let state = new Cache();
    let chain = new Rpc(URL);

    let res = await exe.run(head, tx, state, chain);
    // res.state - object with touched state
    // res.steps - array of trace objects
    ```
    */

    #[wasm_bindgen]
    pub struct Stream {
        receiver: mpsc::Receiver<Step>,
        gas: i32,
        tx: Int,
    }

    #[wasm_bindgen]
    impl Stream {
        pub async fn next(&mut self) -> JsValue {
            match self.receiver.next().await {
                Some(step) => {
                    let value = serde_wasm_bindgen::to_value(&step).unwrap_or(JsValue::NULL);
                    release().await; // yield to JS event loop for smoothiness
                    value
                }
                None => JsValue::NULL,
            }
        }

        pub fn gas(&self) -> i32 {
            self.gas
        }

        pub fn tx(&self) -> JsString {
            self.tx.to_string().into()
        }
    }

    #[wasm_bindgen]
    pub async fn run(url: JsString) -> Result<Stream, JsError> {
        run_inner(url)
            .await
            .map_err(|e| JsError::new(&e.to_string()))
    }

    async fn run_inner(url: JsString) -> eyre::Result<Stream> {
        let rpc = Rpc::latest(url.into()).await?;
        let (call, tx, head): (Call, Tx, Head) = {
            let block = rpc.block(rpc.block_number + 1).await?;
            let tx = &block.txs[0];
            let call = tx.call.clone().into();
            (call, tx.tx.clone(), block.head)
        };
        let hash = tx.hash;

        let (ytx, yrx) = mpsc::channel(1024);
        let mut cache = Cache::with_sender(ytx);

        let mut exe = Executor::new(call);
        let res = exe.run(tx, head, &mut cache, &rpc).await?;
        let _ = cache.sender.take();

        Ok(Stream { receiver: yrx, gas: res.gas().finalized as i32, tx: hash })
    }

    async fn release() {
        let promise = js_sys::Promise::resolve(&JsValue::NULL);
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

/*
TODO:

Call target: execute call against network block at given height.
Call target: execute network tx (given block + index OR tx hash).
Call target: execute network block (given block number/hash).

Call target: bring your own env (bytecode, call, storage, etc).
(Fully reproducible hermetic execution environment, demo/PoC/etc)

In-memory (or remote server) cache of account state and storage.

Traces:
- trace filter (only collect certain event types)
- trace streaming (ease memory pressure, backpressure/pause)

Result:
- show affected accounts (balance, nonce) and storage slots
- show created accounts (if any)
- show emitted logs (if any)
- show gas usage per step/frame

Intelligence:
- resolve function selectors (4byte.directory)
- resolve function parameters (source code + ABI)
- for Solidity source code (matching bytecode + srcmap): per-line debugging
- resolve storage slots based on hash preimage (e.g. ERC-20 token balance for specific address)

---

Use Case: Anomaly Detection and Transaction Verification

Re-execute all previous transactions for the given address, collect results (state, value).
Re-execute target transaction, collect results (state, value) and compare to previous results.
If significant deviation is detected [TODO: define "significant"], flag and report it.

[This could have prevented "ByBit hack" of Feb'25 $1.5B worth of ETH being stolen].
https://www.chainalysis.com/blog/bybit-exchange-hack-february-2025-crypto-security-dprk/
https://www.cremit.io/blog/bybit-hacking-incident-analysis-how-to-strengthen-cryptocurrency-exchange-security

*/
