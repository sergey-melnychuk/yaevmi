pub mod dto;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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
pub struct EmptyChain;

#[async_trait::async_trait]
impl Chain for EmptyChain {
    async fn get(&self, _: &Acc, _: &Int) -> eyre::Result<Int> {
        Ok(Default::default())
    }
    async fn acc(&self, _: &Acc) -> eyre::Result<Account> {
        Ok(Default::default())
    }
    async fn code(&self, _: &Acc) -> eyre::Result<(Buf, Int)> {
        Ok(Default::default())
    }
    async fn nonce(&self, _: &Acc) -> eyre::Result<u64> {
        Ok(Default::default())
    }
    async fn balance(&self, _: &Acc) -> eyre::Result<Int> {
        Ok(Default::default())
    }
    async fn head(&self, _: u64) -> eyre::Result<Head> {
        Ok(Default::default())
    }
    async fn block(&self, number: u64) -> eyre::Result<(Head, Vec<Tx>)> {
        eyre::bail!("EmptyChain: block({number}) not available")
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
        chain_id: Int::from(1u32),
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
    let mut call = build_call(tc, &entry.indexes);

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

    // Access list for this data index (EIP-2930)
    let access_list: Vec<dto::AccessListEntry> = tc
        .transaction
        .access_lists
        .as_deref()
        .unwrap_or_default()
        .get(entry.indexes.data)
        .and_then(|o| o.as_ref())
        .cloned()
        .unwrap_or_default();

    // Intrinsic gas: base + calldata + EIP-3860 initcode + EIP-2930 access list
    let intrinsic: u64 = {
        let base: u64 = if recipient.is_zero() { 53_000 } else { 21_000 };
        let cd: u64 = call
            .data
            .0
            .iter()
            .map(|&b| if b == 0 { 4u64 } else { 16u64 })
            .sum();
        let initcode_cost: u64 = if recipient.is_zero() {
            2 * ((call.data.0.len() as u64 + 31) / 32)
        } else {
            0
        };
        let al_cost: u64 = access_list
            .iter()
            .map(|e| 2400 + e.storage_keys.len() as u64 * 1900)
            .sum();
        base + cd + initcode_cost + al_cost
    };
    // Reject invalid transactions (gasLimit < intrinsic)
    if gas_limit < intrinsic {
        return Ok(());
    }
    call.gas = gas_limit - intrinsic;

    // 1. Increment sender nonce
    let sender_nonce = state.nonce(&sender).unwrap_or(Int::ZERO).as_u64();
    state.inc_nonce(&sender, Int::ONE);

    // 2. Upfront deduction: gas_limit * max_fee + value
    let upfront = U256::from(gas_limit) * max_fee + i2u(value);
    let bal = i2u(state.balance(&sender).unwrap_or(Int::ZERO));
    state.set_value(&sender, u2i(bal - upfront));

    // 3. Determine call mode and handle value transfer / account creation
    let mode = if recipient.is_zero() {
        let addr = yaevmi_core::aux::create_address(&sender, sender_nonce);
        state.create(
            addr,
            yaevmi_core::state::Account {
                value: Int::ZERO,
                nonce: Int::ONE,
                code: (yaevmi_misc::buf::Buf::default(), Int::ZERO),
            },
        );
        if !value.is_zero() {
            let bal = i2u(state.balance(&addr).unwrap_or(Int::ZERO));
            state.set_value(&addr, u2i(bal + i2u(value)));
        }
        yaevmi_core::evm::CallMode::Create(addr)
    } else {
        if !value.is_zero() {
            let bal = i2u(state.balance(&recipient).unwrap_or(Int::ZERO));
            state.set_value(&recipient, u2i(bal + i2u(value)));
        }
        yaevmi_core::evm::CallMode::Call(0, 0)
    };

    // EIP-2929: pre-warm sender, recipient, and access list
    state.warm_acc(&sender);
    if !recipient.is_zero() {
        state.warm_acc(&recipient);
    }
    for al_entry in &access_list {
        state.warm_acc(&al_entry.address);
        for key in &al_entry.storage_keys {
            state.warm_key(&al_entry.address, key);
        }
    }

    // Execute — reverts/halts return Ok(Done { status: 0 }), only infra errors are Err
    let mut exe = Executor::new(call, mode);
    let gas = match exe.run(head, &mut state, &EmptyChain).await {
        Ok(CallResult::Created { addr, code, gas }) => {
            if !code.is_empty() {
                let hash = Int::from(yaevmi_misc::keccak256(&code).as_ref());
                state.acc_mut(&addr).code = (code.into(), hash);
            }
            gas
        }
        Ok(CallResult::Done { gas, .. }) => gas,
        Err(e) => {
            println!("DEBUG: ERROR: {e}");
            yaevmi_core::evm::Gas {
                limit: gas_limit as i64,
                spent: gas_limit as i64,
                refund: 0,
            }
        }
    };

    // Gas accounting (intrinsic + EVM execution)
    let evm_gas = gas_limit.saturating_sub(intrinsic);
    let evm_used = (gas.spent.max(0) as u64).min(evm_gas);
    let gas_used = (intrinsic + evm_used).min(gas_limit);
    let max_refund = gas_used / 5; // EIP-3529
    let gas_refund = (gas.refund.max(0) as u64).min(max_refund);
    let effective_used = gas_used - gas_refund;

    // 4. Refund sender: upfront was gas_limit*max_fee, actual cost is effective_used*gas_price
    let refund_wei = U256::from(gas_limit) * max_fee - U256::from(effective_used) * gas_price;
    let bal = i2u(state.balance(&sender).unwrap_or(Int::ZERO));
    state.set_value(&sender, u2i(bal + refund_wei));

    // 5. Pay coinbase the priority fee (effective_gas_price - base_fee) * gas_used
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
async fn test_shallow_stack() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/GeneralStateTests/stStackTests/shallowStack.json"
    );
    let file: dto::TestFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut failed = 0;
    for (name, tc) in &file {
        for result in run_case(tc, "Cancun").await {
            if let Err(e) = result {
                println!("FAIL: {name}: {e}");
                failed += 1;
            }
        }
    }
    assert!(failed == 0, "shallowStack.json failed");
}

/// Run every test in GeneralStateTests/ for the Cancun fork.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_general_state_cancun() -> eyre::Result<()> {
    const FORK: &str = "Cancun";
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/GeneralStateTests");

    let counter = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for category in std::fs::read_dir(root).expect("GeneralStateTests not found") {
        let category = category.unwrap().path();
        if !category.is_dir() {
            continue;
        }
        // let category_name = category.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // if category_name == "stTimeConsuming" && std::env::var("RUN_TIME_CONSUMING").is_err() {
        //     continue;
        // }
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

            let counter = counter.clone();
            let handle = tokio::spawn(async move {
                let mut total: usize = 0;
                let mut failed = Vec::new();

                // println!("DEBUG: Run: {file_path:?}: {}", file.len());
                for (name, tc) in file {
                    for result in run_case(&tc, FORK).await {
                        total += 1;
                        if let Err(e) = result {
                            failed.push(format!("FAIL: {name}: {e}"));
                        }
                    }
                }
                let count = counter.fetch_add(1, Ordering::Relaxed);
                println!("DEBUG: Done ({count}): {file_path:?}: {total}");
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
    for s in failed.iter().take(take) {
        println!("---\n{s}");
    }
    let left = failed.len() - take;
    if left > 0 {
        println!("(skipped {} more failures)", left);
    }

    println!("\n=== GeneralStateTests/{FORK} ===");
    println!("passed: {}", total - failed.len());
    println!("failed: {}", failed.len());
    println!(" TOTAL: {total}");

    assert!(failed.is_empty(), "GeneralStateTests/{FORK} failed");
    Ok(())
}
