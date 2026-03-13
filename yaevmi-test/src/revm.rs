use revm::bytecode::Bytecode;
use revm::bytecode::opcode::OpCode;
use revm::context::TxEnv;
use revm::context_interface::result::{ExecResultAndState, ExecutionResult, Output};
use revm::database::InMemoryDB;
use revm::inspector::InspectorEvmTr;
use revm::interpreter::interpreter_types::{Immediates, Jumps};
use revm::interpreter::{Interpreter, interpreter::EthInterpreter};
use revm::primitives::{Address, B256, Bytes, TxKind, U256};
use revm::state::AccountInfo;
use revm::{Context, InspectEvm, Inspector, MainBuilder, MainContext};
use yaevmi_base::{Acc, Int};
use yaevmi_core::state::Account;
use yaevmi_core::trace::Step;
use yaevmi_core::{Call, Head};
use yaevmi_misc::buf::Buf;

#[derive(Debug, Default)]
pub struct Tracer {
    pub traces: Vec<Step>,
    step: Option<Step>,
}

impl<CTX> Inspector<CTX, EthInterpreter> for Tracer {
    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, _context: &mut CTX) {
        let pc = interp.bytecode.pc();
        let op = interp.bytecode.opcode();
        let name = OpCode::new(op)
            .map(|op| op.as_str())
            .unwrap_or("UNKNOWN")
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
        });
    }

    fn step_end(&mut self, interp: &mut Interpreter<EthInterpreter>, _context: &mut CTX) {
        let gas = interp.gas.remaining();
        if let Some(mut step) = self.step.take() {
            step.gas = gas;
            step.stack = interp.stack.len();
            step.memory = interp.memory.len();
            self.traces.push(step);
        }
    }
}

pub async fn run(
    call: Call,
    head: Head,
    env: Vec<(Acc, Account, Vec<(Int, Int)>)>,
) -> eyre::Result<(Int, Buf, i64, Vec<Step>)> {
    let to_addr = |a: &Acc| Address::from(<[u8; 20]>::try_from(a.as_ref()).unwrap());
    let to_u256 = |i: &Int| U256::from_be_bytes(<[u8; 32]>::try_from(i.as_ref()).unwrap());
    let to_b256 = |i: &Int| B256::from(<[u8; 32]>::try_from(i.as_ref()).unwrap());

    let mut db = InMemoryDB::default();
    for (acc, account, storage) in &env {
        let addr = to_addr(acc);
        let code = if account.code.0.0.is_empty() {
            None
        } else {
            Some(Bytecode::new_raw(Bytes::from(account.code.0.0.clone())))
        };
        db.insert_account_info(
            addr,
            AccountInfo {
                balance: to_u256(&account.value),
                nonce: account.nonce.as_u64(),
                code_hash: to_b256(&account.code.1),
                code,
                account_id: None,
            },
        );
        for (slot, value) in storage {
            db.insert_account_storage(addr, to_u256(slot), to_u256(value))
                .unwrap();
        }
    }

    let mut ctx = Context::mainnet().with_db(db);
    ctx.block.number = U256::from(head.number);
    ctx.block.timestamp = to_u256(&head.timestamp);
    ctx.block.gas_limit = head.gas_limit.as_u64();
    ctx.block.beneficiary = to_addr(&head.coinbase);
    ctx.block.basefee = head.base_fee.as_u64();
    ctx.block.prevrandao = Some(to_b256(&head.prevrandao));
    ctx.cfg.chain_id = head.chain_id as u64;

    let kind = if call.to.is_zero() {
        TxKind::Create
    } else {
        TxKind::Call(to_addr(&call.to))
    };
    let tx = TxEnv::builder()
        .caller(to_addr(&call.by))
        .kind(kind)
        .gas_limit(call.gas)
        .gas_price(head.gas_price.as_u128())
        .value(to_u256(&call.eth))
        .data(Bytes::from(call.data.0.clone()))
        .nonce(call.nonce.unwrap_or(0))
        .build()
        .map_err(|e| eyre::eyre!("{e:?}"))?;

    let mut evm = ctx.build_mainnet_with_inspector(Tracer::default());
    let ExecResultAndState { result, state: _ } = evm.inspect_tx(tx)?;
    let tracer = std::mem::take(&mut evm.inspector().traces);

    // for (addr, account) in &state {
    //     println!("{addr}");
    //     println!("  value={}", account.info.balance);
    //     println!("  nonce={}", account.info.nonce);
    //     for (slot, val) in &account.storage {
    //         println!("  [{slot}] = {}", val.present_value);
    //     }
    // }

    match result {
        ExecutionResult::Success { gas, output, .. } => match output {
            Output::Create(code, Some(addr)) => Ok((
                Acc::from(addr.as_slice()).to::<32>(),
                Buf(code.to_vec()),
                gas.used() as i64,
                tracer.into(),
            )),
            Output::Create(_, None) => Err(eyre::eyre!("contract creation: no address")),
            Output::Call(bytes) => Ok((
                Int::from(1u64),
                Buf(bytes.to_vec()),
                gas.used() as i64,
                tracer.into(),
            )),
        },
        ExecutionResult::Revert { gas, output, .. } => Ok((
            Int::from(0u64),
            Buf(output.to_vec()),
            gas.used() as i64,
            tracer.into(),
        )),
        ExecutionResult::Halt { gas, .. } => Ok((
            Int::from(0u64),
            Buf::default(),
            gas.used() as i64,
            tracer.into(),
        )),
    }
}
