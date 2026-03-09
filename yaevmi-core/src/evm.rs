use std::ops::Range;

use crate::{Acc, Call, Int, Result, ops::OPS, state::State};

const K: usize = 1024;

pub enum HaltReason {
    OutOfGas,
    BadJump(usize),
    BadOpcode(u8),
    NonStatic,
    StackUnderflow,
}

pub enum Fetch {
    Code(Acc),
    Nonce(Acc),
    Balance(Acc),
    Account(Acc),
    BlockHash(u64),
    StateCell(Acc, Int),
}

pub enum CallMode {
    Call,
    Static,
    Delegate,
    CallCode,
}

pub enum CreateMode {
    Create,
    Create2,
}

pub enum StepResult {
    End,
    Ok { gas_amount: i64, gas_refund: i64 },
    Call(Call, CallMode, Range<usize>),
    Create(Call, CreateMode),
    Return(Vec<u8>),
    Revert(Vec<u8>),
    Halt(HaltReason),
    Fetch(Fetch),
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
    pub is_static: bool,
    pub depth: usize,
    pub this: Acc,
}

pub struct Evm {
    pub pc: usize,
    pub gas: Gas,
    pub stack: Vec<Int>,
    pub memory: Vec<u8>,
    pub code: Vec<u8>,
}

impl Evm {
    pub const STACK_SIZE_LIMIT: usize = 1024;
    pub const MEMORY_SIZE_LIMIT: usize = 4 * K * K;

    pub fn new(code: Vec<u8>, gas: u64) -> Self {
        Self {
            pc: 0,
            gas: Gas::new(gas),
            stack: Vec::with_capacity(Self::STACK_SIZE_LIMIT),
            memory: Vec::with_capacity(4 * K),
            code,
        }
    }

    pub fn pop<const N: usize>(&mut self) -> Option<[Int; N]> {
        let mut ret = [Int::ZERO; N];
        if self.stack.len() < N {
            return None;
        }
        for slot in ret.iter_mut() {
            if let Some(value) = self.stack.pop() {
                *slot = value;
            } else {
                return None;
            }
        }
        Some(ret)
    }

    pub fn step(
        &mut self,
        ctx: &Context,
        call: &Call,
        state: &mut impl State,
    ) -> Result<StepResult> {
        if self.pc >= self.code.len() {
            return Ok(StepResult::End);
        }
        let op = self.code[self.pc];
        let (_name, f) = OPS[op as usize];
        f(self, ctx, call, state)
    }
}
