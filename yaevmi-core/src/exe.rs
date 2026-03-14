use yaevmi_base::math::U256;
use yaevmi_base::math::lift;
use yaevmi_misc::keccak256;

use yaevmi_misc::buf::Buf;

use crate::Tx;
use crate::aux::{create_address, is_precompile};
use crate::evm::{CallMode, Context, Evm, Fetch, Gas, StepResult};
use crate::{Acc, Call, Error, Int, Result};
use crate::{
    call::Head,
    chain::{Chain, fetch},
    state::{Account, State},
};

const MAX_CALL_DEPTH: usize = 1024;
const MAX_STEPS: u64 = 10_000_000;
const MAX_CODE_SIZE: usize = 24_576;
const CODE_DEPOSIT_GAS: i64 = 200;

#[derive(Debug)]
pub enum CallResult {
    Done { status: Int, ret: Buf, gas: Gas },
    Created { addr: Acc, code: Buf, gas: Gas },
}

pub struct Executor {
    pub call: Call,
    pub callstack: Vec<CallFrame>,
}

pub struct CallFrame {
    pub call: Call,
    // pub mode: CallMode,
    pub evm: Evm,
    pub ctx: Context,
    pub checkpoint: usize,
}

pub fn pre_charge(call: &Call) -> i64 {
    let mut total = 21_000i64;
    if call.to.is_zero() {
        total += 32_000;
        // EIP-3860: 2 gas per 32-byte word of initcode
        total += 2 * ((call.data.0.len() as i64 + 31) / 32);
    }
    let zeroes = call.data.0.iter().filter(|b| **b == 0).count();
    let non_zeroes = call.data.0.len() - zeroes;
    total += (zeroes * 4 + non_zeroes * 16) as i64;
    total
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
        tx: Tx,
        head: Head,
        state: &mut impl State,
        chain: &impl Chain,
    ) -> Result<CallResult> {
        if !self.callstack.is_empty() {
            return Err(Error::InconsistentState);
        }

        let mode = if self.call.to.is_zero() {
            // Create by EOA, needs address calculations
            let Some(nonce) = state.nonce(&self.call.by) else {
                return Err(Error::MissingData(Fetch::Nonce(self.call.to)));
            };
            let created = create_address(&self.call.by, nonce.as_u64());
            // Register the new account (nonce=1, EIP-161) before running init code
            state.create(
                created,
                Account {
                    value: Int::ZERO,
                    nonce: Int::ONE,
                    code: (Buf::default(), Int::ZERO),
                },
            );
            // ETH endowment from creator to new contract
            if !self.call.eth.is_zero() {
                let sub = lift(|[a, b]| a - b);
                let add = lift(|[a, b]| a + b);
                let by0 = state.balance(&self.call.by).unwrap_or_default();
                state.set_value(&self.call.by, sub([by0, self.call.eth]));
                let to0 = state.balance(&created).unwrap_or_default();
                state.set_value(&created, add([to0, self.call.eth]));
            }
            CallMode::Create(created)
        } else {
            // ETH value transfer for top-level CALL (harness already deducted value in upfront)
            if !self.call.eth.is_zero() {
                let add = lift(|[a, b]| a + b);
                let to = state.auth(&self.call.to).unwrap_or(self.call.to);
                let to0 = state.balance(&to).unwrap_or_default();
                state.set_value(&to, add([to0, self.call.eth]));
            }
            CallMode::Call(0, 0)
        };

        let pre_charge = pre_charge(&self.call);

        let mut frame = prepare(head, self.call.clone(), mode, None, state, chain).await?;
        let _ = frame.evm.gas_charge(pre_charge); // TODO: check for OOG
        self.callstack.push(frame);

        state.inc_nonce(&self.call.by, Int::ONE);

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
                    ret: vec![].into(),
                    gas,
                });
            }
            // Process a result returned from a completed subcall
            if let Some(call_result) = result.take() {
                match call_result {
                    CallResult::Done { status, ret, gas } => {
                        let _ = this.evm.push(status);
                        this.evm.ret = ret.clone().into_vec();
                        if !status.is_zero() {
                            let (offset, size) = target;
                            if size > 0 {
                                let _ = this.evm.mem_put(offset, size, ret.as_slice());
                            }
                        }
                        let gas_sent = gas.limit - subcall_stipend;
                        let return_gas = (gas.limit - gas.spent).max(0).min(gas_sent);
                        this.evm.gas.spent -= return_gas;
                        this.evm.gas.refund += gas.refund;
                        this.evm.apply(state);
                        this.evm.pc += 1;
                        target = (0, 0);
                        subcall_stipend = 0;
                        result = None;
                    }
                    CallResult::Created { addr, code, gas } => {
                        if !code.0.is_empty() {
                            let hash = Int::from(keccak256(code.as_slice()).as_ref());
                            state.acc_mut(&addr).code = (code, hash);
                        }
                        let _ = this.evm.push(addr.to());
                        let return_gas = (gas.limit - gas.spent).max(0);
                        this.evm.gas.spent -= return_gas;
                        this.evm.gas.refund += gas.refund;
                        this.evm.apply(state);
                        this.evm.pc += 1;
                        this.evm.ret.clear();
                        subcall_stipend = 0;
                    }
                }
            }

            match this.evm.step(&this.ctx, &this.call, state)? {
                StepResult::Ok => {
                    continue;
                }
                StepResult::End => {
                    let is_create = this.call.to.is_zero();
                    let gas = this.evm.gas;
                    result = Some(if is_create {
                        CallResult::Created {
                            addr: this.ctx.this,
                            code: vec![].into(),
                            gas,
                        }
                    } else {
                        CallResult::Done {
                            status: Int::ONE,
                            ret: vec![].into(),
                            gas,
                        }
                    });
                    self.callstack.pop();
                }
                StepResult::Call(call, mode) => {
                    /*fn precompile(
                        evm: &mut Evm,
                        call: &Call,
                        mode: &CallMode,
                    ) -> crate::evm::EvmResult<()> {
                        let (ok, out, gas_used) =
                            crate::pre::run(call.to.as_u64(), &call.data.0, call.gas as i64);
                        if ok {
                            evm.gas_charge(gas_used)?;
                            let (ret_offset, ret_size) = mode.range();
                            let n = ret_size.min(out.len());
                            if n > 0 {
                                evm.mem_put(ret_offset, n, &out[..n])?;
                            }
                            evm.push(Int::ONE)?;
                        } else {
                            evm.gas_charge(call.gas as i64)?;
                            evm.push(Int::ZERO)?;
                        }
                        Ok(())
                    }*/

                    if is_precompile(&call.to) {
                        // Precompile runs inline. Replace child-gas reservation with actual used
                        // (avoids OOG when child_gas > remaining); keep access cost.
                        let (ok, out, gas_used) =
                            crate::pre::run(call.to.as_u64(), &call.data.0, call.gas as i64);
                        this.evm.pending_gas_charge -= call.gas as i64;
                        this.evm.pending_gas_charge += gas_used;
                        let status = if ok { Int::ONE } else { Int::ZERO };
                        let (ret_offset, ret_size) = mode.range();
                        this.evm.apply(state);
                        let _ = this.evm.push(status);
                        if !status.is_zero() && ret_size > 0 {
                            let n = ret_size.min(out.len());
                            let _ = this.evm.mem_put(ret_offset, n, &out[..n]);
                        }
                        this.evm.apply(state);
                        this.evm.pc += 1;
                        continue;
                    }

                    let checkpoint = state.checkpoint(this.ctx.depth);
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
                    let is_create = this.call.to.is_zero();
                    result = Some(if is_create {
                        let deploy_cost = CODE_DEPOSIT_GAS * ret.len() as i64;
                        if ret.len() > MAX_CODE_SIZE || this.evm.gas_remaining() < deploy_cost {
                            this.evm.gas.drain();
                            let gas = this.evm.gas;
                            state.revert_to(this.checkpoint);
                            CallResult::Done {
                                status: Int::ZERO,
                                ret: vec![].into(),
                                gas,
                            }
                        } else {
                            this.evm.gas.spent += deploy_cost;
                            let gas = this.evm.gas;
                            CallResult::Created {
                                addr: this.ctx.this,
                                code: ret.into(),
                                gas,
                            }
                        }
                    } else {
                        let gas = this.evm.gas;
                        CallResult::Done {
                            status: Int::ONE,
                            ret: ret.into(),
                            gas,
                        }
                    });
                    self.callstack.pop();
                }
                StepResult::Revert(ret) => {
                    state.revert_to(this.checkpoint);
                    result = Some(CallResult::Done {
                        status: Int::ZERO,
                        ret: ret.into(),
                        gas: this.evm.gas,
                    });
                    self.callstack.pop();
                }
                StepResult::Halt(_reason) => {
                    this.evm.gas.drain();
                    state.revert_to(this.checkpoint);
                    result = Some(CallResult::Done {
                        status: Int::ZERO,
                        ret: vec![].into(),
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

        // For top-level CREATE, store the deployed bytecode into the new account.
        if let CallResult::Created { addr, ref code, .. } = result
            && !code.0.is_empty()
        {
            let hash = Int::from(keccak256(code.as_slice()).as_ref());
            state.acc_mut(&addr).code = (code.clone(), hash);
        }

        // Charge gas: sender pays net_gas * gas_price; coinbase receives the tip.
        let gas = match &result {
            CallResult::Done { gas, .. } | CallResult::Created { gas, .. } => *gas,
        };
        let effective_refund = gas.refund.min(gas.spent / 5);
        let net_gas = (gas.spent - effective_refund).max(0) as u64;
        let mul = lift(|[a, b]| a * b);
        let sub = lift(|[a, b]| a - b);
        let add = lift(|[a, b]| a + b);
        let gas_cost = mul([Int::from(net_gas), tx.gas_price]);
        let sender_bal = state.balance(&self.call.by).unwrap_or_default();
        state.set_value(&self.call.by, sub([sender_bal, gas_cost]));
        let tip = mul([Int::from(net_gas), sub([tx.gas_price, head.base_fee])]);
        if !tip.is_zero() {
            let cb_bal = state.balance(&head.coinbase).unwrap_or_default();
            state.set_value(&head.coinbase, add([cb_bal, tip]));
        }

        state.apply();
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
    let is_create = call.to.is_zero();
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
    let checkpoint = state.checkpoint(ctx.depth);
    Ok(CallFrame {
        call,
        // mode,
        evm,
        ctx,
        checkpoint,
    })
}
