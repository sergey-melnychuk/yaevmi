use alloy_provider::ProviderBuilder;
use futures::{StreamExt, channel::mpsc};
use yaevmi_base::int;
use yaevmi_core::{
    Call,
    cache::Cache,
    chain::Chain,
    exe::{CallResult, Executor},
    rpc::Rpc,
};

// TODO: 0xcf41706dc2f05b3fd765fac52a1cc0c678f434b264b73cfac2c44f00cfe86ccf: EIP-7702
// TODO: 0xb283062d05a29d6b09a6c903c1ef6bdc82732a852a6ab51a9224110b4ad4e744: EIP-4844

// TODO: 0x50f089afa2cff9c59634ec4973589697f3fe229b44108cf856f436cab0a710ee: wrong gas
// TODO: 0x86b27d44f2c337470e3aa6bc77550940937fe7cd44b656fe468566db4ec9632b: wrong gas

const YAEVMI_RPC_URL: &str = "YAEVMI_RPC_URL";

/*
cargo build --release --bin replay
./target/release/replay >replay.log 2>&1
*/
#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();
    let Ok(url) = std::env::var(YAEVMI_RPC_URL) else {
        eyre::bail!("{YAEVMI_RPC_URL} not set");
    };
    let mut rpc = Rpc::latest(url.clone()).await?;

    let (block, tx, head) = {
        let arg = std::env::args()
            .nth(1)
            .unwrap_or_else(|| String::from("latest:0"));

        if arg.starts_with("0x") {
            let hash = int(&arg); // TODO: handle invalid hex
            let receipt = rpc.receipt(hash).await?;
            let block = receipt.block_number.as_u64();
            let index = receipt.transaction_index.as_u64();
            let tx = rpc.lookup(block, index).await?;
            let head = rpc.head(block).await?;
            (block, tx, head)
        } else if arg.contains(":") {
            let mut split = arg.split(":");
            let block = split.next().unwrap();
            let block: u64 = if block == "latest" {
                rpc.block_number
            } else {
                block.parse().unwrap()
            };
            let index: u64 = split.next().unwrap().parse().unwrap();
            let tx = rpc.lookup(block, index).await?;
            let head = rpc.head(block).await?;
            (block, tx, head)
        } else {
            eyre::bail!("Unexpected arg: '{arg}'.\nExpected: '0x<hash>' or '<block>:<index>'.");
        }
    };
    rpc.reset(block - 1).await?;

    println!("Chain ID: {}", rpc.chain_id().await?);
    println!("Block Hash: {}", rpc.block_hash);
    println!("Block Number: {}", rpc.block_number);

    let hash = tx.tx.hash;
    let call: Call = tx.call.into();
    let tx = tx.tx.clone();
    println!("Tx Hash:  {}", tx.hash);
    println!("Tx Index: {}", tx.index);

    let (ytx, mut yrx) = mpsc::channel(1024 * 1024);
    let mut cache = Cache::with_sender(ytx);

    let (rtx, mut rrx) = mpsc::channel(1024 * 1024);

    let handle = tokio::spawn(async move {
        let is_trace = std::env::var("TRACE").is_ok();
        if is_trace {
            println!("---\nSTREAMING OPENED");
        }
        let mut skip = 0;
        loop {
            let y = yrx.next().await;
            let r = rrx.next().await;
            if let (Some(mut y), Some(mut r)) = (y, r) {
                if y != r {
                    println!("===\nSTEP MISMATCH:\nYEVM: {y:#?}\nREVM: {r:#?}\n(skip: {skip})");
                    break;
                }
                if is_trace {
                    for line in r.debug.drain(..) {
                        y.debug.push(format!("REVM: {line}"));
                    }
                    println!("{y:#?}");
                }
                skip += 1;
            } else {
                break;
            }
        }
        if is_trace {
            println!("STREAMING CLOSED [{skip} items]\n---");
        }
    });

    let pack = (call.clone(), tx.clone(), head.clone());

    let provider = ProviderBuilder::new()
        // .connect("https://mainnet.infura.io/v3/c60b0bb42f8a4c6481ecd229eddaca27")
        .connect(&url)
        .await
        .unwrap();

    tokio::task::spawn_blocking(move || {
        let (call, tx, head) = pack;
        let _ = live::run(call, tx, head, rtx, provider);
    });

    let mut exe = Executor::new(call);
    let result = exe.run(tx, head, &mut cache, &rpc).await?;

    let _ = cache.sender.take();

    handle.await?;
    println!("RESULT: {:#?}", result);

    let receipt = rpc.receipt(hash).await?;
    let used_gas = receipt.gas_used.as_u64() as i64;
    match result {
        CallResult::Done {
            status,
            ret: _,
            gas,
        } => {
            assert_eq!(gas.finalized, used_gas, "gas");
            assert_eq!(status, receipt.status, "status");
        }
        CallResult::Created { acc, code: _, gas } => {
            assert_eq!(gas.finalized, used_gas, "gas");
            assert_eq!(Some(acc), receipt.contract_address, "created");
        }
    }
    println!("OK: {hash}");
    Ok(())
}

// TODO: run embedded database for acc/state storage
// consider: sqlite, leveldb, rocksdb, sled, yakvdb?

// TODO: for each processed block: generate hermetic env
// (containing all read storage cells by all transactions)
// (store it alongsize with block updates to allow reverting)
// (this allows re-running blocks on-demand without RPC calls)

mod live {
    use alloy_provider::Provider;
    use revm::bytecode::opcode::OpCode;
    use revm::context::transaction::{AccessList, AccessListItem};
    use revm::context::{ContextTr, TxEnv};
    use revm::context_interface::result::ExecResultAndState;
    use revm::database::{AlloyDB, BlockId, CacheDB, WrapDatabaseAsync};
    use revm::interpreter::interpreter_types::{Immediates, Jumps};
    use revm::interpreter::{CallInputs, CallOutcome, CreateInputs, CreateOutcome};
    use revm::interpreter::{Interpreter, interpreter::EthInterpreter};
    use revm::primitives::{Address, B256, Bytes, TxKind, U256};
    use revm::{Context, InspectEvm, Inspector, MainBuilder, MainContext};

    use futures::channel::mpsc;
    use yaevmi_base::{Acc, Int};
    use yaevmi_core::trace::Step;
    use yaevmi_core::{Call, Head, Tx};
    use yaevmi_misc::buf::Buf;

    #[derive(Debug, Default)]
    pub struct Tracer {
        step: Option<Step>,
        refund: i64,
        gas: u64,
        depth: usize,
        tx: Option<mpsc::Sender<Step>>,
    }

    impl<CTX: ContextTr> Inspector<CTX, EthInterpreter> for Tracer {
        fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, _ctx: &mut CTX) {
            let pc = interp.bytecode.pc();
            let op = interp.bytecode.opcode();
            let name = OpCode::new(op)
                .map(|op| op.as_str())
                .unwrap_or("INVALID")
                .to_owned();
            let data = if (0x60..=0x7f).contains(&op) {
                let n = (op - 0x60 + 1) as usize;
                let raw = interp.bytecode.read_slice(n + 1);
                Some(Buf(raw[1..].to_vec()))
            } else {
                None
            };

            let gas = interp.gas.remaining();
            let stack = interp.stack.len();
            let memory = interp.memory.len();
            self.step = Some(Step {
                pc,
                op,
                name,
                data,
                gas,
                stack,
                memory,
                debug: vec![],
            });
            self.gas = gas;

            if op == 0x55
                && let (Ok(key), Ok(val)) = (interp.stack.peek(0), interp.stack.peek(1))
                && let Some(step) = self.step.as_mut()
            {
                step.debug.push(format!("SSTORE: key={key:0x}"));
                step.debug.push(format!("SSTORE: val={val:0x}"));
            }
        }

        fn step_end(&mut self, interp: &mut Interpreter<EthInterpreter>, _ctx: &mut CTX) {
            let gas = interp.gas.remaining();
            let cost = self.gas - gas;

            let refund = interp.gas.refunded() - self.refund;
            self.refund = interp.gas.refunded();

            if let Some(mut step) = self.step.take() {
                step.gas = gas;
                step.stack = interp.stack.len();
                step.memory = interp.memory.len();
                step.debug.push(format!("cost={cost}"));
                if refund > 0 {
                    step.debug.push(format!("refund={refund}"));
                }
                step.debug.push(format!("depth={}", self.depth));
                if let Some(tx) = self.tx.as_mut() {
                    let _ = tx.try_send(step); // TODO: check for error
                }
            }
        }

        fn call(&mut self, _: &mut CTX, _: &mut CallInputs) -> Option<CallOutcome> {
            self.depth += 1;
            None
        }

        fn call_end(&mut self, _: &mut CTX, _: &CallInputs, _: &mut CallOutcome) {
            self.depth -= 1;
        }

        fn create(&mut self, _: &mut CTX, _: &mut CreateInputs) -> Option<CreateOutcome> {
            self.depth += 1;
            None
        }

        fn create_end(&mut self, _: &mut CTX, _: &CreateInputs, _: &mut CreateOutcome) {
            self.depth -= 1;
        }

        fn selfdestruct(&mut self, _: Address, _: Address, _: U256) {
            self.depth -= 1;
        }
    }

    pub fn run(
        call: Call,
        tx: Tx,
        head: Head,
        sender: mpsc::Sender<Step>,
        provider: impl Provider + Clone,
    ) -> eyre::Result<()> {
        let to_addr = |a: &Acc| Address::from(<[u8; 20]>::try_from(a.as_ref()).unwrap());
        let to_u256 = |i: &Int| U256::from_be_bytes(<[u8; 32]>::try_from(i.as_ref()).unwrap());
        let to_b256 = |i: &Int| B256::from(<[u8; 32]>::try_from(i.as_ref()).unwrap());

        let db = AlloyDB::new(provider, BlockId::from(to_b256(&head.parent_hash)));
        let db = WrapDatabaseAsync::new(db).unwrap();
        let db = CacheDB::new(db);

        let mut ctx = Context::mainnet().with_db(db);
        ctx.block.number = U256::from(head.number.as_u64());
        ctx.block.timestamp = to_u256(&head.timestamp);
        ctx.block.gas_limit = head.gas_limit.as_u64();
        ctx.block.beneficiary = to_addr(&head.coinbase);
        ctx.block.basefee = head.base_fee.as_u64();
        ctx.block.prevrandao = Some(to_b256(&head.prevrandao));
        ctx.cfg.chain_id = tx.chain_id.as_u64();

        // For legacy tx (max_fee_per_gas=0), use gas_price for effective fee
        let max_fee = if tx.max_fee_per_gas.is_zero() {
            tx.gas_price.as_u128()
        } else {
            tx.max_fee_per_gas.as_u128()
        };
        let priority_fee = if tx.max_fee_per_gas.is_zero() {
            tx.gas_price.as_u128()
        } else {
            tx.max_priority_fee_per_gas.as_u128()
        };

        let kind = if call.is_create() {
            TxKind::Create
        } else {
            TxKind::Call(to_addr(&call.to))
        };
        let tx = TxEnv::builder()
            .caller(to_addr(&call.by))
            .kind(kind)
            .gas_limit(call.gas)
            .gas_price(tx.gas_price.as_u128())
            .value(to_u256(&call.eth))
            .data(Bytes::from(call.data.0.clone()))
            .nonce(tx.nonce.as_u64())
            .access_list(AccessList::from(
                tx.access_list
                    .iter()
                    .map(|item| AccessListItem {
                        address: to_addr(&item.address),
                        storage_keys: item.storage_keys.iter().map(to_b256).collect::<Vec<B256>>(),
                    })
                    .collect::<Vec<AccessListItem>>(),
            ))
            .max_fee_per_gas(max_fee)
            .gas_priority_fee(Some(priority_fee))
            .authorization_list(vec![])
            .blob_hashes(vec![])
            .max_fee_per_blob_gas(Int::ZERO.as_u128())
            .build()
            .map_err(|e| eyre::eyre!("{e:?}"))?;

        let inspector = Tracer {
            tx: Some(sender),
            ..Tracer::default()
        };
        let mut evm = ctx.build_mainnet_with_inspector(inspector);
        let ExecResultAndState {
            result: _,
            state: _,
        } = evm.inspect_tx(tx)?;
        let _ = evm.inspector.tx.take();
        Ok(())
    }
}
