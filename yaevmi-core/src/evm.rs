use std::ops::Range;

use yaevmi_base::dto::Head;

use crate::{Acc, Call, Int, Result, ops::OPS, state::State};

const K: usize = 1024;

#[derive(Debug)]
pub enum HaltReason {
    OutOfGas,
    OutOfMemory,
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

#[derive(Debug)]
pub enum CallMode {
    Call(Range<usize>),
    Static(Range<usize>),
    Delegate(Range<usize>),
    CallCode(Range<usize>),
    Create(Acc),
    Create2(Acc),
}

impl CallMode {
    pub fn range(&self) -> Range<usize> {
        match self {
            Self::Call(r) => r.clone(),
            Self::Static(r) => r.clone(),
            Self::Delegate(r) => r.clone(),
            Self::CallCode(r) => r.clone(),
            _ => 0..0,
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
    Ok { gas_amount: i64, gas_refund: i64 },
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
        }
    }

    pub fn popn_usize<const N: usize>(&mut self) -> EvmResult<[usize; N]> {
        let mut ret = [0usize; N];
        let pop = self.popn::<N>()?;
        for (i, item) in ret.iter_mut().enumerate() {
            *item = pop[i].as_usize();
        }
        Ok(ret)
    }

    pub fn popn<const N: usize>(&mut self) -> EvmResult<[Int; N]> {
        let mut ret = [Int::ZERO; N];
        if self.stack.len() < N {
            return Err(EvmYield::Halt(HaltReason::StackUnderflow));
        }
        for slot in ret.iter_mut() {
            if let Some(value) = self.stack.pop() {
                *slot = value;
            } else {
                return Err(EvmYield::Halt(HaltReason::StackUnderflow));
            }
        }
        Ok(ret)
    }

    pub fn push(&mut self, int: Int) -> EvmResult<()> {
        if self.stack.len() >= Self::STACK_SIZE_LIMIT {
            return Err(EvmYield::Halt(HaltReason::StackOverflow));
        }
        self.stack.push(int);
        Ok(())
    }

    pub fn mem_put(&mut self, target: Range<usize>, source: &[u8]) -> EvmResult<i64> {
        let (len, lo, hi) = (target.len(), target.start, target.end);
        let cap = self.memory.capacity();
        let end = (lo + source.len()).min(hi);
        if end > Evm::MEMORY_SIZE_LIMIT {
            return Err(EvmYield::Halt(HaltReason::OutOfMemory));
        }
        let cost = if end > self.memory.len() {
            if end > cap {
                self.memory.reserve(cap - end);
            }
            // TODO: calculate memory expansion costs
            0
        } else {
            0
        };
        let take = source.len().min(len);
        let padding = source.len().max(len) - source.len();
        self.memory[lo..hi].copy_from_slice(&source[..take]);
        for i in lo..hi + padding {
            self.memory[i] = 0;
        }
        Ok(cost)
    }

    pub fn mem_get(&self, target: Range<usize>) -> EvmResult<(&[u8], usize)> {
        let (lo, hi) = (target.start, target.end.max(self.memory.len()));
        let padding = hi - self.memory.len();
        Ok((&self.memory[lo..hi], padding))
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
        self.pc += 1;
        result
            .map(|(gas_amount, gas_refund)| StepResult::Ok {
                gas_amount,
                gas_refund,
            })
            .or_else(|evm_yield| {
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
