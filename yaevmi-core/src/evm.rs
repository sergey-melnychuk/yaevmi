use serde::{Deserialize, Serialize};
use yaevmi_base::dto::Head;

use crate::{Acc, Call, Int, Result, ops::OPS, state::State};

const K: usize = 1024;

#[derive(Debug, Deserialize, Serialize)]
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
}

impl Gas {
    pub fn new(gas: u64) -> Self {
        Self {
            limit: gas as i64,
            spent: 0,
            refund: 0,
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

    pub fn take(&mut self, gas: i64) -> EvmResult<i64> {
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
    pop: usize,
    mem_cost: i64,
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
            pop: 0,
            mem_cost: 0,
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
        self.pop = N;
        Ok(ret)
    }

    pub fn pull(&mut self) -> EvmResult<()> {
        if self.stack.len() < self.pop {
            return Err(EvmYield::Halt(HaltReason::StackUnderflow));
        }
        for _ in 0..self.pop {
            let _ = self.stack.pop();
        }
        self.pop = 0;
        Ok(())
    }

    pub fn push(&mut self, int: Int) -> EvmResult<()> {
        self.pull()?;
        if self.stack.len() >= Self::STACK_SIZE_LIMIT {
            return Err(EvmYield::Halt(HaltReason::StackOverflow));
        }
        self.stack.push(int);
        Ok(())
    }

    pub fn mem_expand(&mut self, offset: usize, size: usize) -> EvmResult<()> {
        mem_check(offset, size)?;
        let end = offset + size;
        if end > Evm::MEMORY_SIZE_LIMIT {
            return Err(EvmYield::Halt(HaltReason::OutOfMemory));
        }
        if end > self.memory.len() {
            self.memory.resize(end, 0);
            let words = end.div_ceil(32) as i64;
            let new_cost = words * words / 512 + 3 * words;
            let cost = new_cost - self.mem_cost;
            self.mem_cost = new_cost;
            self.gas.take(cost)?;
        }
        Ok(())
    }

    pub fn mem_put(&mut self, offset: usize, size: usize, source: &[u8]) -> EvmResult<()> {
        self.mem_expand(offset, size)?;
        let copy_len = source.len().min(size);
        self.memory[offset..offset + copy_len].copy_from_slice(&source[..copy_len]);
        Ok(())
    }

    pub fn mem_get(&self, offset: usize, size: usize) -> EvmResult<(&[u8], usize)> {
        mem_check(offset, size)?;
        let (lo, hi) = (
            offset.min(self.memory.len()),
            (offset + size).min(self.memory.len()),
        );
        let pad = hi.max(self.memory.len()) - self.memory.len();
        Ok((&self.memory[lo..hi], pad))
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
        let (_name, f) = OPS[op as usize];
        let result = f(self, ctx, call, state);
        result.map(|_| StepResult::Ok).or_else(|evm_yield| {
            Ok(match evm_yield {
                EvmYield::Halt(reason) => StepResult::Halt(reason),
                EvmYield::Fetch(fetch) => StepResult::Fetch(fetch),
                EvmYield::Return(ret) => StepResult::Return(ret),
                EvmYield::Revert(ret) => StepResult::Revert(ret),
                EvmYield::Call(call, mode) => StepResult::Call(call, mode),
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
