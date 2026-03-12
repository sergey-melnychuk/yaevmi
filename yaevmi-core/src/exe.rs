use yaevmi_base::math::lift;
use yaevmi_base::{dto::Head, math::U256};
use yaevmi_misc::keccak256;

use yaevmi_misc::buf::Buf;

use crate::evm::{CallMode, Context, Evm, Gas, StepResult};
use crate::{Acc, Call, Error, Int, Result};
use crate::{
    chain::{Chain, fetch},
    state::{Account, State},
};

const MAX_CALL_DEPTH: usize = 1024;
const MAX_STEPS: u64 = 1_000_000;
const MAX_CODE_SIZE: usize = 24_576;
const CODE_DEPOSIT_GAS: i64 = 200;

pub enum CallResult {
    Done { status: Int, ret: Vec<u8>, gas: Gas },
    Created { addr: Acc, code: Vec<u8>, gas: Gas },
}

pub struct Executor {
    pub call: Call,
    pub mode: CallMode,
    pub callstack: Vec<CallFrame>,
}

pub struct CallFrame {
    pub call: Call,
    pub mode: CallMode,
    pub evm: Evm,
    pub ctx: Context,
    pub checkpoint: usize,
}

impl Executor {
    pub fn new(call: Call, mode: CallMode) -> Self {
        Self {
            call,
            mode,
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
            let frame = prepare(head, self.call.clone(), self.mode, None, state, chain).await?;
            self.callstack.push(frame);
        }

        let mut target: (usize, usize) = (0, 0);
        let mut subcall_stipend: i64 = 0;
        let mut result: Option<CallResult> = None;
        let mut steps: u64 = 0;

        while let Some(this) = self.callstack.last_mut() {
            steps += 1;
            if steps > MAX_STEPS {
                this.evm.gas.drain();
                let gas = this.evm.gas;
                let checkpoint = this.checkpoint;
                state.revert_to(checkpoint);
                self.callstack.clear();
                return Ok(CallResult::Done {
                    status: Int::ZERO,
                    ret: vec![],
                    gas,
                });
            }
            // Process a result returned from a completed subcall
            if let Some(call_result) = result.take() {
                match call_result {
                    CallResult::Done { status, ret, gas } => {
                        let _ = this.evm.push(status);
                        this.evm.ret = ret.clone();
                        if !status.is_zero() {
                            let (offset, size) = target;
                            if size > 0 {
                                let _ = this.evm.mem_put(offset, size, &ret);
                            }
                        }
                        let gas_sent = gas.limit - subcall_stipend;
                        let return_gas = (gas.limit - gas.spent).max(0).min(gas_sent);
                        this.evm.gas.spent -= return_gas;
                        this.evm.gas.refund += gas.refund;
                        target = (0, 0);
                        subcall_stipend = 0;
                        result = None;
                    }
                    CallResult::Created { addr, code, gas } => {
                        if !code.is_empty() {
                            let hash = Int::from(keccak256(&code).as_ref());
                            state.acc_mut(&addr).code = (code.into(), hash);
                        }
                        let _ = this.evm.push(addr.to());
                        let return_gas = (gas.limit - gas.spent).max(0);
                        this.evm.gas.spent -= return_gas;
                        this.evm.gas.refund += gas.refund;
                        this.evm.ret.clear();
                        subcall_stipend = 0;
                    }
                }
            }

            match this.evm.step(&this.ctx, &this.call, state)? {
                StepResult::Ok => {
                    this.evm.apply(state);
                    this.evm.pc += 1;
                    continue;
                }
                StepResult::End => {
                    let is_create = matches!(this.mode, CallMode::Create(_) | CallMode::Create2(_));
                    let gas = this.evm.gas;
                    result = Some(if is_create {
                        CallResult::Created {
                            addr: this.ctx.this,
                            code: vec![],
                            gas,
                        }
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
                    subcall_stipend = if !call.eth.is_zero()
                        && matches!(mode, CallMode::Call(..) | CallMode::CallCode(..))
                    {
                        2300
                    } else {
                        0
                    };

                    let is_create = matches!(mode, CallMode::Create(_) | CallMode::Create2(_));

                    if is_create {
                        let creator = call.by;
                        let addr = mode.acc();

                        // Increment creator's nonce
                        state.inc_nonce(&creator, Int::ONE);

                        // Collision check: existing nonce or code at derived address
                        let existing_nonce = state.nonce(&addr).unwrap_or(Int::ZERO);
                        let has_code = state.code(&addr).is_some_and(|(c, _)| !c.0.is_empty());
                        if !existing_nonce.is_zero() || has_code {
                            state.revert_to(checkpoint);
                            let _ = this.evm.push(Int::ZERO);
                            this.evm.ret = vec![];
                            target = (0, 0);
                            continue;
                        }

                        // Create account with nonce=1 (EIP-161)
                        state.create(
                            addr,
                            Account {
                                value: Int::ZERO,
                                nonce: Int::ONE,
                                code: (Buf::default(), Int::ZERO),
                            },
                        );

                        // Value transfer from creator to new account
                        if !call.eth.is_zero() {
                            let gte = lift(|[a, b]| if a >= b { U256::ONE } else { U256::ZERO });
                            let by0 = state.balance(&creator).unwrap_or_default();
                            if gte([by0, call.eth]).is_zero() {
                                state.revert_to(checkpoint);
                                let _ = this.evm.push(Int::ZERO);
                                this.evm.ret = vec![];
                                target = (0, 0);
                                continue;
                            }
                            let sub = lift(|[a, b]| a - b);
                            let add = lift(|[a, b]| a + b);
                            state.set_value(&creator, sub([by0, call.eth]));
                            let to0 = state.balance(&addr).unwrap_or_default();
                            state.set_value(&addr, add([to0, call.eth]));
                        }
                    }

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
                    if !is_create
                        && !call.eth.is_zero()
                        && matches!(mode, CallMode::Call(..) | CallMode::CallCode(..))
                    {
                        let by = call.by;
                        let to = state.auth(&call.to).unwrap_or(call.to);
                        let by0 = state.balance(&by).unwrap_or_default();

                        let gte = lift(|[a, b]| if a >= b { U256::ONE } else { U256::ZERO });
                        if gte([by0, call.eth]).is_zero() {
                            state.revert_to(checkpoint);
                            let _ = this.evm.push(Int::ZERO);
                            this.evm.ret = vec![];
                            target = (0, 0);
                            continue;
                        }

                        let add = lift(|[a, b]| a + b);
                        let sub = lift(|[a, b]| a - b);
                        let to0 = state.balance(&to).unwrap_or_default();
                        state.set_value(&by, sub([by0, call.eth]));
                        state.set_value(&to, add([to0, call.eth]));
                    }
                    self.callstack.push(frame);
                }
                StepResult::Return(ret) => {
                    let is_create = matches!(this.mode, CallMode::Create(_) | CallMode::Create2(_));
                    result = Some(if is_create {
                        let deploy_cost = CODE_DEPOSIT_GAS * ret.len() as i64;
                        if ret.len() > MAX_CODE_SIZE || this.evm.gas.remaining() < deploy_cost {
                            this.evm.gas.drain();
                            let gas = this.evm.gas;
                            state.revert_to(this.checkpoint);
                            CallResult::Done {
                                status: Int::ZERO,
                                ret: vec![],
                                gas,
                            }
                        } else {
                            this.evm.gas.spent += deploy_cost;
                            let gas = this.evm.gas;
                            CallResult::Created {
                                addr: this.ctx.this,
                                code: ret,
                                gas,
                            }
                        }
                    } else {
                        let gas = this.evm.gas;
                        CallResult::Done {
                            status: Int::ONE,
                            ret,
                            gas,
                        }
                    });
                    self.callstack.pop();
                }
                StepResult::Revert(ret) => {
                    state.revert_to(this.checkpoint);
                    result = Some(CallResult::Done {
                        status: Int::ZERO,
                        ret,
                        gas: this.evm.gas,
                    });
                    self.callstack.pop();
                }
                StepResult::Halt(_reason) => {
                    this.evm.gas.drain();
                    state.revert_to(this.checkpoint);
                    result = Some(CallResult::Done {
                        status: Int::ZERO,
                        ret: vec![],
                        gas: this.evm.gas,
                    });
                    self.callstack.pop();
                }
                StepResult::Fetch(f) => {
                    fetch(f, state, chain).await?;
                    this.evm.reset();
                }
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
    let is_create = matches!(mode, CallMode::Create(_) | CallMode::Create2(_));
    let code = if is_create {
        call.data.clone()
    } else if let Some((code, _)) = state.code(&call.to) {
        code
    } else if let Ok((code, hash)) = chain.code(&call.to).await {
        state.acc_mut(&call.to).code = (code.clone(), hash);
        code
    } else {
        Buf::default()
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
        mode,
        evm,
        ctx,
        checkpoint: state.checkpoint(),
    })
}
