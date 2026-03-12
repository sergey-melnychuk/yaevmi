use yaevmi_base::math::lift;
use yaevmi_base::{dto::Head, math::U256};

use crate::evm::{CallMode, Context, Evm, Gas, StepResult};
use crate::{Acc, Call, Error, Int, Result};
use crate::{
    chain::{Chain, fetch},
    state::State,
};

const MAX_CALL_DEPTH: usize = 1024;

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
    pub checkpoint: usize,
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
                CallMode::Call(0, 0),
                None,
                state,
                chain,
            )
            .await?;
            self.callstack.push(frame);
        }

        let mut target: (usize, usize) = (0, 0);
        let mut result: Option<CallResult> = None;

        while let Some(this) = self.callstack.last_mut() {
            // Process a result returned from a completed subcall
            if let Some(call_result) = result.take() {
                match call_result {
                    CallResult::Done { status, ret, gas } => {
                        let _ = this.evm.push(status);
                        this.evm.ret = ret.clone();
                        if !status.is_zero() {
                            // Success: write return data into parent memory
                            let (offset, size) = target;
                            if size > 0 {
                                let _ = this.evm.mem_put(offset, size, &ret);
                            }
                        }
                        this.evm.gas.spent += gas.spent;
                        this.evm.gas.refund += gas.refund;
                        target = (0, 0);
                        result = None;
                    }
                    CallResult::Created(acc, gas) => {
                        let _ = this.evm.push(acc.to());
                        this.evm.gas.spent += gas.spent;
                        this.evm.gas.refund += gas.refund;
                        this.evm.ret.clear();
                    }
                }
            }

            match this.evm.step(&this.ctx, &this.call, state)? {
                StepResult::Ok => continue,
                StepResult::End => {
                    // == STOP == RETURN []
                    let (is_create, gas) = (this.call.to.is_zero(), this.evm.gas);
                    result = Some(if is_create {
                        CallResult::Created(this.ctx.this, gas)
                    } else {
                        CallResult::Done {
                            status: Int::ONE,
                            ret: vec![],
                            gas,
                        }
                    });
                    self.callstack.pop();
                }
                StepResult::Call(call, mode) => {
                    let checkpoint = state.checkpoint();
                    target = mode.range();
                    let frame =
                        prepare(head, call.clone(), mode, Some(&this.ctx), state, chain).await?;
                    if frame.ctx.depth > MAX_CALL_DEPTH {
                        state.revert_to(checkpoint);
                        let _ = this.evm.push(Int::ZERO);
                        this.evm.ret = vec![];
                        target = (0, 0);
                        continue;
                    }

                    // ETH value transfer for CALL and CALLCODE
                    if !call.eth.is_zero()
                        && matches!(mode, CallMode::Call(..) | CallMode::CallCode(..))
                    {
                        let by = call.by;
                        let to = state.auth(&call.to).unwrap_or(call.to);
                        let to0 = state.balance(&to).unwrap_or_default();
                        let by0 = state.balance(&by).unwrap_or_default();

                        let gte = lift(|[a, b]| if a >= b { U256::ONE } else { U256::ZERO });
                        let sufficient = !gte([by0, to0]).is_zero();
                        if !sufficient {
                            // Insufficient balance — fail without executing subcall
                            let _ = this.evm.push(Int::ZERO);
                            this.evm.ret = vec![];
                            target = (0, 0);
                            continue;
                        }

                        let add = lift(|[a, b]| a + b);
                        let sub = lift(|[a, b]| a - b);

                        let by1 = sub([by0, call.eth]);
                        let to1 = add([to0, call.eth]);

                        state.set_value(&by, by1);
                        state.set_value(&to, to1);
                    }
                    self.callstack.push(frame);
                }
                StepResult::Return(ret) => {
                    let (is_create, gas) = (this.call.to.is_zero(), this.evm.gas);
                    result = Some(if is_create {
                        CallResult::Created(this.ctx.this, gas)
                    } else {
                        CallResult::Done {
                            status: Int::ONE,
                            ret,
                            gas,
                        }
                    });
                    self.callstack.pop();
                }
                StepResult::Revert(ret) => {
                    let (checkpoint, gas) = (this.checkpoint, this.evm.gas);
                    state.revert_to(checkpoint);
                    result = Some(CallResult::Done {
                        status: Int::ZERO,
                        ret,
                        gas,
                    });
                    self.callstack.pop();
                }
                StepResult::Halt(_reason) => {
                    let checkpoint = this.checkpoint;
                    this.evm.gas.drain();
                    let gas = this.evm.gas;
                    state.revert_to(checkpoint);
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
    } else if let Some((code, _)) = state.code(&call.to) {
        code
    } else {
        let (code, hash) = chain.code(&call.to).await?;
        state.acc_mut(&call.to).code = (code.clone(), hash);
        code
    };
    let evm = Evm::new(head, code.into_vec(), call.gas);
    let is_static = matches!(mode, CallMode::Static(_, _));
    let this = match mode {
        CallMode::Create(acc) => acc,
        CallMode::Create2(acc) => acc,
        CallMode::Call(_, _) | CallMode::Static(_, _) => call.to,
        CallMode::CallCode(_, _) | CallMode::Delegate(_, _) => {
            ctx.map(|c| c.this).unwrap_or(call.by)
        }
    };
    let ctx = if let Some(ctx) = ctx {
        Context {
            origin: ctx.origin,
            is_static: ctx.is_static || is_static,
            depth: ctx.depth + 1,
            this,
        }
    } else {
        Context {
            origin: call.by,
            is_static,
            depth: 1,
            this,
        }
    };
    Ok(CallFrame {
        call,
        evm,
        ctx,
        checkpoint: state.checkpoint(),
    })
}
