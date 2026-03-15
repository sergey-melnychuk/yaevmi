#[cfg(target_arch = "wasm32")] // comment-out this line for development
mod wasm {
    use js_sys::JsString;
    use wasm_bindgen::prelude::*;
    use yaevmi_core::Call;

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
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
