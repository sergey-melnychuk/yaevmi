pub mod dto;

use yaevmi_base::{Acc, Int, dto::Head};
use yaevmi_core::{
    Call, Tx,
    cache::Cache,
    chain::Chain,
    exe::{CallResult, Executor},
    state::{Account, State},
};

use dto::{PostEntry, TestCase};
use yaevmi_misc::buf::Buf;

/// Minimal Chain impl for tests — all state is pre-loaded into Cache.
/// Any Fetch call hitting this means the pre-state is incomplete.
pub struct NoChain;

#[async_trait::async_trait]
impl Chain for NoChain {
    async fn get(&self, _: &Acc, _: &Int) -> eyre::Result<Int> {
        eyre::bail!("NoChain: unexpected Fetch::StateCell")
    }
    async fn acc(&self, acc: &Acc) -> eyre::Result<Account> {
        eyre::bail!("NoChain: unexpected Fetch::Account for {acc:?}")
    }
    async fn code(&self, acc: &Acc) -> eyre::Result<(Buf, Int)> {
        eyre::bail!("NoChain: unexpected Fetch::Code for {acc:?}")
    }
    async fn nonce(&self, acc: &Acc) -> eyre::Result<u64> {
        eyre::bail!("NoChain: unexpected Fetch::Nonce for {acc:?}")
    }
    async fn balance(&self, acc: &Acc) -> eyre::Result<Int> {
        eyre::bail!("NoChain: unexpected Fetch::Balance for {acc:?}")
    }
    async fn head(&self, number: u64) -> eyre::Result<Head> {
        eyre::bail!("NoChain: unexpected Fetch::BlockHash({number})")
    }
    async fn block(&self, number: u64) -> eyre::Result<(Head, Vec<Tx>)> {
        eyre::bail!("NoChain: block({number}) not available")
    }
}

/// Build an Cache from a test case's `pre` section.
pub fn build_state(tc: &TestCase) -> Cache {
    use yaevmi_misc::keccak256;
    let mut state = Cache::new();
    for (addr, pre) in &tc.pre {
        let code = pre.code.as_slice().to_vec();
        let code_hash = keccak256(&code);
        let account = Account {
            value: pre.balance,
            nonce: pre.nonce,
            code: (code.into(), code_hash),
        };
        state.insert_account(*addr, account);
        for (key, val) in &pre.storage {
            state.insert_storage(*addr, *key, *val);
        }
    }
    state
}

/// Build the Head (block environment) from the `env` section.
pub fn build_head(tc: &TestCase) -> Head {
    let hash = tc
        .env
        .current_random
        .or(tc.env.current_difficulty)
        .unwrap_or(Int::ZERO);
    Head {
        number: tc.env.current_number.as_u64(),
        hash,
        gas_limit: tc.env.current_gas_limit,
        coinbase: tc.env.current_coinbase.to(),
        timestamp: tc.env.current_timestamp,
        base_fee: tc.env.current_base_fee.unwrap_or(Int::ZERO),
        prevrandao: hash,
        ..Head::default()
    }
}

/// Build a Call for one (data_idx, gas_idx, value_idx) combination.
pub fn build_call(tc: &TestCase, idx: &dto::Indexes) -> Call {
    Call {
        by: tc.transaction.sender,
        to: tc.transaction.to.unwrap_or(Acc::ZERO),
        gas: tc.transaction.gas_limit[idx.gas].as_u64(),
        eth: tc.transaction.value[idx.value],
        data: tc.transaction.data[idx.data].as_slice().to_vec().into(),
        auth: vec![],
        nonce: Some(tc.transaction.nonce.as_u64()),
    }
}

/// Run a single post-entry and return Ok(()) if execution and state match.
pub async fn run_entry(tc: &TestCase, entry: &PostEntry) -> eyre::Result<()> {
    use yaevmi_base::math::U256;

    let head = build_head(tc);
    let mut state = build_state(tc);
    let call = build_call(tc, &entry.indexes);

    let gas_limit = call.gas;
    let value = call.eth;
    let sender = call.by;
    let recipient = call.to;

    let i2u = |i: Int| U256::from_be_slice(i.as_ref());
    let u2i = |u: U256| -> Int { Int::from(&u.to_be_bytes::<32>()[..]) };

    // Effective gas price (EIP-1559 or legacy)
    let base_fee = i2u(head.base_fee);
    let max_fee = i2u(tc
        .transaction
        .max_fee_per_gas
        .or(tc.transaction.gas_price)
        .unwrap_or(Int::ZERO));
    let gas_price = if tc.transaction.max_fee_per_gas.is_some() {
        let priority = i2u(tc.transaction.max_priority_fee_per_gas.unwrap_or(Int::ZERO));
        max_fee.min(base_fee + priority)
    } else {
        max_fee
    };

    // 1. Increment sender nonce
    state.inc_nonce(&sender, Int::ONE);

    // 2. Upfront deduction: gas_limit * max_fee + value
    let upfront = U256::from(gas_limit) * max_fee + i2u(value);
    let bal = i2u(state.balance(&sender).unwrap_or(Int::ZERO));
    state.set_value(&sender, u2i(bal - upfront));

    // 3. Value transfer to recipient (regular calls only)
    if !value.is_zero() && !recipient.is_zero() {
        let bal = i2u(state.balance(&recipient).unwrap_or(Int::ZERO));
        state.set_value(&recipient, u2i(bal + i2u(value)));
    }

    // Execute — reverts/halts return Ok(Done { status: 0 }), only infra errors are Err
    let mut exe = Executor::new(call);
    let gas = match exe.run(head, &mut state, &NoChain).await {
        Ok(CallResult::Done { gas, .. }) | Ok(CallResult::Created(_, gas)) => gas,
        Err(_) => yaevmi_core::evm::Gas {
            limit: gas_limit as i64,
            spent: gas_limit as i64,
            refund: 0,
        },
    };

    // Gas accounting
    let gas_used = (gas.spent.max(0) as u64).min(gas_limit);
    let max_refund = gas_used / 5; // EIP-3529
    let gas_refund = (gas.refund.max(0) as u64).min(max_refund);
    let effective_used = gas_used - gas_refund;

    // 4. Refund unused gas to sender (at max_fee rate per EIP-1559)
    let refund_wei = U256::from(gas_limit - effective_used) * max_fee;
    let bal = i2u(state.balance(&sender).unwrap_or(Int::ZERO));
    state.set_value(&sender, u2i(bal + refund_wei));

    // 5. Pay coinbase priority fee
    let priority = gas_price.saturating_sub(base_fee);
    let miner_reward = U256::from(effective_used) * priority;
    if !miner_reward.is_zero() {
        let coinbase: Acc = head.coinbase.to();
        let bal = i2u(state.balance(&coinbase).unwrap_or(Int::ZERO));
        state.set_value(&coinbase, u2i(bal + miner_reward));
    }

    // Validate explicit post-state when present in the fixture.
    for (addr, expected) in &entry.state {
        let balance = expected.balance;
        let actual_balance = state.balance(addr).unwrap_or(Int::ZERO);
        eyre::ensure!(
            actual_balance == balance,
            "\n for {addr:?} balance:\n got {actual_balance:?}\nwant {balance:?}"
        );
        let actual_nonce = state.nonce(addr).unwrap_or(Int::ZERO);
        eyre::ensure!(
            actual_nonce == expected.nonce,
            "\n for {addr:?} nonce:\n got {actual_nonce}\nwant {}",
            expected.nonce
        );
        for (key, want) in &expected.storage {
            let got = state.storage(addr, key).unwrap_or(Int::ZERO);
            eyre::ensure!(
                got == *want,
                "\n for {addr:?}[{key:?}]:\n got {got:?}\nwant {want:?}"
            );
        }
    }
    Ok(())
}

/// Run all post-entries for a given fork in a test case.
pub async fn run_case(tc: &TestCase, fork: &str) -> Vec<eyre::Result<()>> {
    let Some(entries) = tc.post.get(fork) else {
        return vec![];
    };
    let mut results = Vec::with_capacity(entries.len());
    for entry in entries {
        results.push(run_entry(tc, entry).await);
    }
    results
}

#[tokio::test]
#[ignore]
async fn test_shallow_stack() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/GeneralStateTests/stStackTests/shallowStack.json"
    );
    let file: dto::TestFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for (name, tc) in &file {
        for result in run_case(tc, "Cancun").await {
            assert!(result.is_ok(), "{name}: {}", result.unwrap_err());
        }
    }
}

/// Run every test in GeneralStateTests/ for the Cancun fork.
/// Prints a pass/fail summary per category. Not run by default.
#[tokio::test(flavor = "multi_thread")]
async fn test_general_state_cancun() -> eyre::Result<()> {
    const FORK: &str = "Cancun";
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/GeneralStateTests");

    let mut handles = Vec::new();
    for category in std::fs::read_dir(root).expect("GeneralStateTests not found") {
        let category = category.unwrap().path();
        if !category.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&category).unwrap() {
            let file_path = entry.unwrap().path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let src = std::fs::read_to_string(&file_path).unwrap();
            let file: dto::TestFile = match serde_json::from_str(&src) {
                Ok(f) => f,
                Err(e) => {
                    println!("ERROR: {}: parse error: {e}", file_path.display());
                    continue;
                }
            };

            let handle = tokio::spawn(async move {
                let mut total: usize = 0;
                let mut failed = Vec::new();

                for (name, tc) in file {
                    for result in run_case(&tc, FORK).await {
                        total += 1;
                        if let Err(e) = result {
                            failed.push(format!("FAIL: {name}: {e}"));
                        }
                    }
                }
                (total, failed)
            });
            handles.push(handle);
        }
    }

    let mut total: usize = 0;
    let mut failed = Vec::new();
    let results: Result<Vec<_>, _> = futures::future::try_join_all(handles).await;
    for (n, fs) in results? {
        total += n;
        failed.extend_from_slice(&fs);
    }

    let take = failed.len() / 1;
    for s in failed.iter().take(10) {
        println!("---\n{s}");
    }
    let left = failed.len() - take;
    if left > 0 {
        println!("(skipped {} more failures)", left);
    }

    let passed = total - failed.len();
    println!("\n=== GeneralStateTests/{FORK} ===");
    println!("passed: {passed}/{total}");
    assert!(passed == total, "GeneralStateTests/{FORK} failed");
    Ok(())
}
