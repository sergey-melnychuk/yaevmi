use std::{borrow::Cow, ops::Range};

use crate::evm::{CallMode, Context, CreateMode, Evm, Gas, StepResult};
use crate::state::{Chain, State, fetch};
use crate::{Acc, Call, Error, Int, Result};

pub enum CallResult {
    Call { status: Int, ret: Vec<u8>, gas: Gas },
    Created(Acc, Gas),
}

pub struct Executor {
    pub callstack: Vec<CallFrame>,
}

pub struct CallFrame {
    pub call: Call,
    pub evm: Evm,
    pub ctx: Context,
}

impl Executor {
    pub async fn step(&mut self, state: &mut impl State, chain: &impl Chain) -> Result<()> {
        let mut target: Range<usize> = 0..0;
        let mut result: Option<CallResult> = None;
        while let Some(this) = self.callstack.last_mut() {
            if let Some(r) = result.take() {
                match r {
                    CallResult::Call { status, ret, gas } => {
                        this.evm.stack.push(status);
                        if !status.is_zero() && !ret.is_empty() {
                            // TODO: check for memory/result resizing
                            this.evm.memory[target].copy_from_slice(&ret);
                        }
                        this.evm.gas.spent += gas.spent;
                        this.evm.gas.refund += gas.refund;

                        target = 0..0;
                        result = None;
                    }
                    CallResult::Created(acc, gas) => {
                        this.evm.stack.push(acc.to_int());
                        this.evm.gas.spent += gas.spent;
                        this.evm.gas.refund += gas.refund;
                    }
                }
            }
            match this.evm.step(&this.ctx, &this.call, state)? {
                StepResult::End => break,
                StepResult::Ok {
                    gas_amount: _,
                    gas_refund: _,
                } => {
                    // TODO: tracing (if enabled): EVM state, gas state, debug info
                    continue;
                }
                StepResult::Call(call, mode, dst) => {
                    target = dst;
                    let (code, _) = state.code(&call.to).ok_or_else(|| {
                        let msg = format!("code missing for {:?}", call.to);
                        Error::Internal(Cow::<str>::Owned(msg))
                    })?;
                    let evm = Evm::new(code, call.gas);
                    let ctx = Context {
                        is_static: this.ctx.is_static
                            || matches!(mode, CallMode::Static | CallMode::CallCode),
                        depth: this.ctx.depth + 1,
                        this: if matches!(mode, CallMode::Call | CallMode::Static) {
                            call.to
                        } else {
                            this.ctx.this
                        },
                    };
                    self.callstack.push(CallFrame { call, evm, ctx });
                }
                StepResult::Create(call, mode) => {
                    let evm = Evm::new(call.data.clone(), call.gas);
                    let created = match mode {
                        CreateMode::Create => Acc::ZERO,
                        CreateMode::Create2 => Acc::ZERO,
                    };
                    let ctx = Context {
                        is_static: this.ctx.is_static,
                        depth: this.ctx.depth + 1,
                        this: created,
                    };
                    self.callstack.push(CallFrame { call, evm, ctx });
                }
                StepResult::Return(ret) => {
                    result = if this.call.to.is_zero() {
                        Some(CallResult::Created(this.ctx.this, this.evm.gas))
                    } else {
                        Some(CallResult::Call {
                            status: Int::ONE,
                            ret,
                            gas: this.evm.gas,
                        })
                    };
                    self.callstack.pop();
                }
                StepResult::Revert(ret) => {
                    // TODO: handle revert
                    result = Some(CallResult::Call {
                        status: Int::ZERO,
                        ret,
                        gas: this.evm.gas,
                    });
                    self.callstack.pop();
                }
                StepResult::Halt(_reason) => {
                    // TODO: handle revert
                    result = Some(CallResult::Call {
                        status: Int::ZERO,
                        ret: vec![],
                        gas: this.evm.gas,
                    });
                    self.callstack.pop();
                }
                StepResult::Fetch(f) => fetch(f, state, chain).await?,
            }
        }
        Ok(())
    }
}
