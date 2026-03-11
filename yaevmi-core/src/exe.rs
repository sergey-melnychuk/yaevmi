use std::ops::Range;

use yaevmi_base::dto::Head;

use crate::evm::{CallMode, Context, Evm, Gas, StepResult};
use crate::state::{Chain, State, fetch};
use crate::{Acc, Call, Error, Int, Result};

pub enum CallResult {
    Done { status: Int, ret: Vec<u8>, gas: Gas },
    Created(Acc, Gas),
}

pub struct Executor {
    pub call: Call,
    pub callstack: Vec<CallFrame>,
}

pub struct CallFrame {
    pub call: Call,
    pub evm: Evm,
    pub ctx: Context,
}

impl Executor {
    pub fn new(call: Call) -> Self {
        Self {
            call,
            callstack: vec![],
        }
    }

    pub async fn run(
        &mut self,
        head: Head,
        state: &mut impl State,
        chain: &impl Chain,
    ) -> Result<CallResult> {
        if self.callstack.is_empty() {
            let frame = prepare(
                head,
                self.call.clone(),
                CallMode::Call(0..0),
                None,
                state,
                chain,
            )
            .await?;
            self.callstack.push(frame);
        }
        let mut target: Range<usize> = 0..0;
        let mut result: Option<CallResult> = None;
        while let Some(this) = self.callstack.last_mut() {
            if let Some(call_result) = result.take() {
                match call_result {
                    CallResult::Done { status, ret, gas } => {
                        this.evm.stack.push(status);
                        if !status.is_zero() && !ret.is_empty() {
                            match this.evm.mem_put(target, &ret) {
                                Ok(gas) => gas,
                                Err(_reason) => {
                                    target = 0..0;
                                    result = Some(CallResult::Done {
                                        status: Int::ZERO,
                                        ret: vec![],
                                        gas: this.evm.gas,
                                    });
                                    self.callstack.pop();
                                    continue;
                                }
                            };
                        }
                        this.evm.gas.spent += gas.spent;
                        this.evm.gas.refund += gas.refund;
                        this.evm.ret = ret;
                        target = 0..0;
                        result = None;
                    }
                    CallResult::Created(acc, gas) => {
                        this.evm.stack.push(acc.to());
                        this.evm.gas.spent += gas.spent;
                        this.evm.gas.refund += gas.refund;
                        this.evm.ret.clear();
                    }
                }
            }
            match this.evm.step(&this.ctx, &this.call, state)? {
                StepResult::Ok => continue,
                StepResult::End => break,
                StepResult::Call(call, mode) => {
                    target = mode.range();
                    let frame = prepare(head, call, mode, Some(&this.ctx), state, chain).await?;
                    self.callstack.push(frame);
                }
                StepResult::Return(ret) => {
                    result = if this.call.to.is_zero() {
                        Some(CallResult::Created(this.ctx.this, this.evm.gas))
                    } else {
                        Some(CallResult::Done {
                            status: Int::ONE,
                            ret,
                            gas: this.evm.gas,
                        })
                    };
                    self.callstack.pop();
                }
                StepResult::Revert(ret) => {
                    // TODO: handle revert
                    result = Some(CallResult::Done {
                        status: Int::ZERO,
                        ret,
                        gas: this.evm.gas,
                    });
                    self.callstack.pop();
                }
                StepResult::Halt(_reason) => {
                    // Halts (exceptions) consume all remaining gas — no refund
                    let gas = Gas {
                        spent: this.evm.gas.limit,
                        refund: 0,
                        ..this.evm.gas
                    };
                    result = Some(CallResult::Done {
                        status: Int::ZERO,
                        ret: vec![],
                        gas,
                    });
                    self.callstack.pop();
                }
                StepResult::Fetch(f) => fetch(f, state, chain).await?,
            }
        }

        let result = result.ok_or(Error::CallResultMissing)?;
        Ok(result)
    }
}

async fn prepare(
    head: Head,
    call: Call,
    mode: CallMode,
    ctx: Option<&Context>,
    state: &mut impl State,
    chain: &impl Chain,
) -> Result<CallFrame> {
    let code = if call.to.is_zero() {
        call.data.clone()
    } else if let Some(code) = state.code(&call.to).map(|(code, _)| code) {
        code
    } else {
        // TODO: check delegation?
        let (code, _) = chain.code(&call.to).await?;
        code
    };
    let evm = Evm::new(head, code, call.gas);
    let ctx = if let Some(ctx) = ctx {
        let this = match mode {
            CallMode::Create(acc) => acc,
            CallMode::Create2(acc) => acc,
            CallMode::Call(_) | CallMode::Static(_) => call.to,
            CallMode::CallCode(_) | CallMode::Delegate(_) => ctx.this,
        };
        Context {
            origin: ctx.origin,
            is_static: ctx.is_static || matches!(mode, CallMode::Static(_) | CallMode::CallCode(_)),
            depth: ctx.depth + 1,
            this,
        }
    } else {
        Context {
            origin: call.by,
            is_static: matches!(mode, CallMode::Static(_) | CallMode::CallCode(_)),
            depth: 1,
            this: call.to,
        }
    };
    Ok(CallFrame { call, evm, ctx })
}
