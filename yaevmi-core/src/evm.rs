use crate::call::Head;
use serde::{Deserialize, Serialize};

use crate::{Acc, Call, Int, Result, ops::OPS, state::State};

const K: usize = 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum HaltReason {
    OutOfGas,
    OutOfMemory,
    BadCopyRange,
    BadJump(usize),
    BadOpcode(u8),
    NonStatic,
    StackUnderflow,
    StackOverflow,
}

#[derive(Debug)]
pub enum Fetch {
    Code(Acc),
    Nonce(Acc),
    Balance(Acc),
    Account(Acc),
    BlockHash(u64),
    StateCell(Acc, Int),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum CallMode {
    Call(usize, usize),
    Static(usize, usize),
    Delegate(usize, usize),
    CallCode(usize, usize),
    Create(Acc),
    Create2(Acc),
}

impl CallMode {
    pub fn range(&self) -> (usize, usize) {
        match self {
            Self::Call(offset, size) => (*offset, *size),
            Self::Static(offset, size) => (*offset, *size),
            Self::Delegate(offset, size) => (*offset, *size),
            Self::CallCode(offset, size) => (*offset, *size),
            _ => (0, 0),
        }
    }

    pub fn acc(&self) -> Acc {
        match self {
            Self::Create(acc) => *acc,
            Self::Create2(acc) => *acc,
            _ => Acc::ZERO,
        }
    }
}

pub enum StepResult {
    End,
    Ok,
    Call(Call, CallMode),
    Return(Vec<u8>),
    Revert(Vec<u8>),
    Halt(HaltReason),
    Fetch(Fetch),
}

impl From<HaltReason> for StepResult {
    fn from(reason: HaltReason) -> Self {
        StepResult::Halt(reason)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Gas {
    pub limit: i64,
    pub spent: i64,
    pub refund: i64,
    pub finalized: i64,
}

impl Gas {
    pub fn new(gas: u64) -> Self {
        Self {
            limit: gas as i64,
            spent: 0,
            refund: 0,
            finalized: 0,
        }
    }

    pub fn remaining(&self) -> i64 {
        self.limit + self.refund - self.spent
    }

    pub fn refund(&mut self, gas: i64) -> EvmResult<()> {
        let rem = self.remaining();
        if rem + gas >= 0 {
            self.refund += gas;
            Ok(())
        } else {
            self.spent += rem;
            Err(EvmYield::Halt(HaltReason::OutOfGas))
        }
    }

    pub fn charge(&mut self, gas: i64) -> EvmResult<i64> {
        let rem = self.remaining();
        if rem >= gas {
            self.spent += gas;
            Ok(rem - gas)
        } else {
            self.spent += rem;
            Err(EvmYield::Halt(HaltReason::OutOfGas))
        }
    }

    pub fn drain(&mut self) {
        self.spent = self.limit;
        self.refund = 0;
    }

    pub fn finalize(&mut self) -> i64 {
        // TODO: final gas calculations
        self.finalized
    }
}

pub struct Context {
    pub origin: Acc,
    pub is_static: bool,
    pub depth: usize,
    pub this: Acc,
}

#[derive(Debug)]
pub enum EvmYield {
    Halt(HaltReason),
    Fetch(Fetch),
    Return(Vec<u8>),
    Revert(Vec<u8>),
    Call(Call, CallMode),
}

pub type EvmResult<T> = std::result::Result<T, EvmYield>;

pub struct Evm {
    pub pc: usize,
    pub gas: Gas,
    pub stack: Vec<Int>,
    pub memory: Vec<u8>,
    pub code: Vec<u8>,
    pub head: Head,
    pub ret: Vec<u8>,

    pub(crate) pending_stack_pops: usize,
    pub(crate) pending_stack_push: Vec<Int>,
    pub(crate) pending_gas_charge: i64,
    pub(crate) pending_gas_refund: i64,
    pub(crate) pending_acc_warmup: Vec<Acc>,
    pub(crate) pending_key_warmup: Vec<(Acc, Int)>,
    pub(crate) pending_mem_stores: Vec<(usize, usize, Vec<u8>)>,
}

impl Evm {
    pub const STACK_SIZE_LIMIT: usize = 1024;
    pub const MEMORY_SIZE_LIMIT: usize = 4 * K * K;

    pub fn new(head: Head, code: Vec<u8>, gas: u64) -> Self {
        Self {
            pc: 0,
            gas: Gas::new(gas),
            stack: Vec::with_capacity(Self::STACK_SIZE_LIMIT),
            memory: Vec::with_capacity(4 * K),
            code,
            head,
            ret: Vec::new(),
            pending_stack_pops: 0,
            pending_stack_push: Vec::new(),
            pending_gas_charge: 0,
            pending_gas_refund: 0,
            pending_mem_stores: Vec::new(),
            pending_acc_warmup: Vec::new(),
            pending_key_warmup: Vec::new(),
        }
    }

    pub fn peek_usize<const N: usize>(&mut self) -> EvmResult<[usize; N]> {
        let mut ret = [0usize; N];
        let pop = self.peek::<N>()?;
        for (i, item) in ret.iter_mut().enumerate() {
            *item = pop[i].as_usize();
        }
        Ok(ret)
    }

    pub fn peek<const N: usize>(&mut self) -> EvmResult<[Int; N]> {
        let mut ret = [Int::ZERO; N];
        if self.stack.len() < N {
            return Err(EvmYield::Halt(HaltReason::StackUnderflow));
        }
        for (slot, value) in ret.iter_mut().zip(self.stack.iter().rev()) {
            *slot = *value;
        }
        self.pending_stack_pops = N;
        Ok(ret)
    }

    pub fn apply(&mut self, state: &mut impl State) {
        for _ in 0..self.pending_stack_pops {
            let _ = self.stack.pop();
        }
        self.pending_stack_pops = 0;

        for int in self.pending_stack_push.drain(..) {
            self.stack.push(int);
        }
        assert!(self.pending_stack_push.is_empty());

        self.gas.spent += self.pending_gas_charge;
        self.pending_gas_charge = 0;

        self.gas.refund += self.pending_gas_refund;
        self.pending_gas_refund = 0;

        for acc in self.pending_acc_warmup.drain(..) {
            state.warm_acc(&acc);
        }
        assert!(self.pending_acc_warmup.is_empty());

        for (acc, key) in self.pending_key_warmup.drain(..) {
            state.warm_key(&acc, &key);
        }
        assert!(self.pending_key_warmup.is_empty());

        let pending_mem_stores = std::mem::take(&mut self.pending_mem_stores);
        for (offset, size, source) in pending_mem_stores {
            self.mem_store(offset, size, source);
        }
    }

    pub fn reset(&mut self) {
        self.pending_stack_pops = 0;
        self.pending_stack_push.clear();
        self.pending_gas_charge = 0;
        self.pending_gas_refund = 0;
        self.pending_mem_stores.clear();
        self.pending_acc_warmup.clear();
        self.pending_key_warmup.clear();
    }

    pub fn push(&mut self, int: Int) -> EvmResult<()> {
        if self.stack.len() >= Self::STACK_SIZE_LIMIT {
            return Err(EvmYield::Halt(HaltReason::StackOverflow));
        }
        self.pending_stack_push.push(int);
        Ok(())
    }

    pub fn warm_acc(&mut self, acc: &Acc) {
        self.pending_acc_warmup.push(*acc);
    }

    pub fn warm_key(&mut self, acc: &Acc, key: &Int) {
        self.pending_key_warmup.push((*acc, *key));
    }

    pub fn gas_remaining(&self) -> i64 {
        self.gas.remaining() - self.pending_gas_charge
    }

    pub fn gas_charge(&mut self, gas: i64) -> EvmResult<()> {
        if gas > self.gas_remaining() {
            return Err(EvmYield::Halt(HaltReason::OutOfGas));
        }
        self.pending_gas_charge += gas;
        Ok(())
    }

    pub fn gas_refund(&mut self, gas: i64) -> EvmResult<()> {
        if self.gas.remaining() + self.gas.refund + gas < 0 {
            return Err(EvmYield::Halt(HaltReason::OutOfGas));
        }
        self.pending_gas_refund += gas;
        Ok(())
    }

    fn mem_expand(&mut self, offset: usize, size: usize) -> EvmResult<()> {
        mem_check(offset, size)?;
        let len = self.memory.len();
        let end = (offset + size).div_ceil(32) * 32;
        if end > len {
            let old_words = (len / 32) as i64;
            let new_words = (end / 32) as i64;
            let cost = (new_words * new_words / 512 + 3 * new_words)
                - (old_words * old_words / 512 + 3 * old_words);
            self.gas_charge(cost)?; // check gas first
            self.memory.resize(end, 0); // then expand
        }
        Ok(())
    }

    fn mem_store(&mut self, offset: usize, size: usize, source: Vec<u8>) {
        let _ = self.mem_expand(offset, size);
        let copy_len = source.len().min(size);
        self.memory[offset..offset + copy_len].copy_from_slice(&source[..copy_len]);
    }

    pub fn mem_put(&mut self, offset: usize, size: usize, source: &[u8]) -> EvmResult<()> {
        self.mem_expand(offset, size)?;
        self.pending_mem_stores
            .push((offset, size, source.to_vec()));
        Ok(())
    }

    pub fn mem_get(&mut self, offset: usize, size: usize) -> EvmResult<Vec<u8>> {
        self.mem_expand(offset, size)?;
        let lo = offset.min(self.memory.len());
        let hi = (offset + size).min(self.memory.len());
        let mut ret = vec![0u8; size];
        ret[..hi - lo].copy_from_slice(&self.memory[lo..hi]);
        Ok(ret)
    }

    pub fn data(&self, pc: usize) -> &[u8] {
        let op = self.code[pc];
        let len = match op {
            0x60..0x80 => op as usize - 0x60 + 1, // PUSH{1..32}
            _ => 0,
        };
        let lo = (pc + 1).min(self.code.len());
        let hi = (pc + 1 + len).min(self.code.len());
        &self.code[lo..hi]
    }

    pub fn step(
        &mut self,
        ctx: &Context,
        call: &Call,
        state: &mut impl State,
    ) -> Result<StepResult> {
        let Some(op) = self.code.get(self.pc).copied() else {
            return Ok(StepResult::End);
        };
        let (name, f) = OPS[op as usize];

        use crate::trace::{Event, Step};
        let pc = self.pc;
        let op = self.code[pc];
        let name = name.to_string();
        let data = self.data(pc);
        let data = if data.is_empty() {
            None
        } else {
            Some(data.to_vec().into())
        };
        let gas = self.gas.remaining().max(0) as u64;
        let mut step = Step {
            pc,
            op,
            name,
            data,
            gas,
            stack: self.stack.len(),
            memory: self.memory.len(),
        };
        let mut step1 = step.clone();

        let result = f(self, ctx, call, state);
        result
            .map(|_| {
                self.apply(state);
                if !is_jump(op) {
                    self.pc += 1;
                }
                let gas = self.gas.remaining().max(0) as u64;
                step.gas = gas;
                step.stack = self.stack.len();
                step.memory = self.memory.len();
                state.emit(Event::Step(step));
                StepResult::Ok
            })
            .or_else(|evm_yield| {
                Ok(match evm_yield {
                    EvmYield::Fetch(fetch) => StepResult::Fetch(fetch),
                    EvmYield::Halt(reason) => {
                        // println!("DEBUG: HALT: {reason:?}");
                        StepResult::Halt(reason)
                    }
                    EvmYield::Return(ret) => {
                        self.apply(state);
                        let gas = self.gas.remaining().max(0) as u64;
                        step1.gas = gas;
                        step1.stack = self.stack.len();
                        step1.memory = self.memory.len();
                        state.emit(Event::Step(step1));
                        StepResult::Return(ret)
                    }
                    EvmYield::Revert(ret) => {
                        self.apply(state);
                        let gas = self.gas.remaining().max(0) as u64;
                        step1.gas = gas;
                        step1.stack = self.stack.len();
                        step1.memory = self.memory.len();
                        state.emit(Event::Step(step1));
                        StepResult::Revert(ret)
                    }
                    EvmYield::Call(call, mode) => {
                        let gas = self.gas.remaining().max(0) as u64;
                        step1.gas = gas;
                        step1.stack = self.stack.len();
                        step1.memory = self.memory.len();
                        state.emit(Event::Step(step1));
                        StepResult::Call(call, mode)
                    }
                })
            })
    }
}

pub fn mem_check(offset: usize, size: usize) -> EvmResult<()> {
    if size < Evm::MEMORY_SIZE_LIMIT && offset <= Evm::MEMORY_SIZE_LIMIT - size {
        return Ok(());
    }
    Err(EvmYield::Halt(HaltReason::OutOfMemory))
}

fn is_jump(op: u8) -> bool {
    op == 0x56 || op == 0x57
}
