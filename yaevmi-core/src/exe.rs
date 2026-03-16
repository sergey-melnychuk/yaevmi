use yaevmi_base::math::U256;
use yaevmi_base::math::lift;
use yaevmi_misc::keccak256;

use yaevmi_misc::buf::Buf;

use crate::Tx;
use crate::aux::{create_address, is_precompile};
use crate::evm::{CallMode, Context, Evm, Gas, StepResult};
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
    Created { acc: Acc, code: Buf, gas: Gas },
}

impl CallResult {
    pub fn gas(&self) -> &Gas {
        match self {
            Self::Done { gas, .. } => gas,
            Self::Created { gas, .. } => gas,
        }
    }

    pub fn gas_mut(&mut self) -> &mut Gas {
        match self {
            Self::Done { gas, .. } => gas,
            Self::Created { gas, .. } => gas,
        }
    }
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

pub fn intrinsic(call: &Call, tx: &Tx, head: &Head, state: &mut impl State) -> (i64, Int) {
    let mut total = 21_000i64;
    if call.is_create() {
        total += 32_000;
        // EIP-3860: 2 gas per 32-byte word of initcode
        total += 2 * ((call.data.0.len() as i64 + 31) / 32);
    }
    let zeroes = call.data.0.iter().filter(|b| **b == 0).count();
    let non_zeroes = call.data.0.len() - zeroes;
    total += (zeroes * 4 + non_zeroes * 16) as i64;

    // EIP-2929: pre-warm sender, target, and coinbase
    state.warm_acc(&call.by);
    state.warm_acc(&call.to);
    state.warm_acc(&head.coinbase.to());

    // EIP-2930: access list gas (2400/address + 1900/storage key)
    for (acc, keys) in &tx.access_list {
        total += 2_400 + 1_900 * keys.len() as i64;
        state.warm_acc(acc);
        for key in keys {
            state.warm_key(acc, key);
        }
    }

    // EIP-1559: effective gas price = min(max_fee_per_gas, base_fee + max_priority_fee_per_gas).
    // For legacy tx (max_fee_per_gas == 0) use gas_price directly.
    let effective_gas_price = if tx.max_fee_per_gas.is_zero() {
        tx.gas_price
    } else {
        let min = lift(|[a, b]| a.min(b));
        let sum = lift(|[a, b]| a + b);
        min([
            tx.max_fee_per_gas,
            sum([head.base_fee, tx.max_priority_fee_per_gas]),
        ])
    };

    // Upfront gas deduction (YP §6.1): sender pays gas_limit × effective_gas_price.
    let mul = lift(|[a, b]| a * b);
    let sub = lift(|[a, b]| a - b);
    let upfront = mul([Int::from(call.gas), effective_gas_price]);
    let balance = state.balance(&call.by).unwrap_or_default();
    if upfront > balance {
        // TODO: return error
    }
    state.set_value(&call.by, sub([balance, upfront]));

    // EIP-7702: authorization list gas (25000/auth tuple)
    total += 25_000 * tx.authorization_list.len() as i64;
    (total, effective_gas_price)
}

pub fn finalized(
    call: &Call,
    tx: &Tx,
    head: &Head,
    effective_gas_price: Int,
    result: &CallResult,
    state: &mut impl State,
) -> i64 {
    // Settle gas: return unused gas to sender; coinbase receives the priority fee tip.
    let gas = result.gas();

    let effective_refund = gas.refund.min(gas.spent / 5);
    let final_gas = (gas.spent - effective_refund).max(0) as u64;
    let returned_gas = (gas.limit.max(0) as u64).saturating_sub(final_gas);
    let mul = lift(|[a, b]| a * b);
    let sub = lift(|[a, b]| a - b);
    let add = lift(|[a, b]| a + b);

    // Return (gas_limit - net_gas) × effective_gas_price to sender.
    let returned_cost = mul([Int::from(returned_gas), effective_gas_price]);
    let balance = state.balance(&call.by).unwrap_or_default();
    state.set_value(&call.by, add([balance, returned_cost]));

    // Priority fee to coinbase: net_gas × min(max_priority_fee, effective_gas_price - base_fee).
    let priority_fee = if tx.max_fee_per_gas.is_zero() {
        sub([effective_gas_price, head.base_fee])
    } else {
        lift(|[a, b]| a.min(b))([
            tx.max_priority_fee_per_gas,
            sub([effective_gas_price, head.base_fee]),
        ])
    };

    let tip = mul([Int::from(final_gas), priority_fee]);
    if !tip.is_zero() {
        let balance = state.balance(&head.coinbase).unwrap_or_default();
        state.set_value(&head.coinbase, add([balance, tip]));
    }
    final_gas as i64
}

pub fn transfer(call: &Call, mode: &CallMode, state: &mut impl State) {
    if call.eth.is_zero() {
        return;
    }
    if let Some(created) = mode.created() {
        let sub = lift(|[a, b]| a - b);
        let add = lift(|[a, b]| a + b);
        let by0 = state.balance(&call.by).unwrap_or_default();
        state.set_value(&call.by, sub([by0, call.eth]));
        let to0 = state.balance(&created).unwrap_or_default();
        state.set_value(&created, add([to0, call.eth]));
    } else {
        let sub = lift(|[a, b]| a - b);
        let add = lift(|[a, b]| a + b);
        let by0 = state.balance(&call.by).unwrap_or_default();
        state.set_value(&call.by, sub([by0, call.eth]));
        let to = state.auth(&call.to).unwrap_or(call.to);
        let to0 = state.balance(&to).unwrap_or_default();
        state.set_value(&to, add([to0, call.eth]));
    }
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
            return Err(Error::Internal("inconsistent state detected".into()));
        }

        let mode = if self.call.is_create() {
            let nonce = state.nonce(&self.call.by).unwrap_or_default();
            let created = create_address(&self.call.by, nonce.as_u64());
            CallMode::Create(created)
        } else {
            CallMode::Call(0, 0)
        };

        let (intrinsic, effective_gas_price) = intrinsic(&self.call, &tx, &head, state);
        if (self.call.gas as i64) < intrinsic {
            return Err(Error::GasTooLow { have: self.call.gas, want: intrinsic as u64 });
        }
        state.inc_nonce(&self.call.by, Int::ONE);

        // prepare() takes a checkpoint to be able to revert,
        // so all state mutations must come AFTER that to be included.
        let mut frame = prepare(tx.clone(), head.clone(), self.call.clone(), mode, None, state, chain).await?;
        // For top-level CREATE: collision check + initialize with nonce=1 (EIP-161).
        // Done AFTER the checkpoint so it's reverted on init-code failure.
        if let CallMode::Create(created) = mode {
            let existing_nonce = state.nonce(&created).unwrap_or(Int::ZERO);
            let has_code = state.code(&created).is_some_and(|(c, _)| !c.0.is_empty());
            if !existing_nonce.is_zero() || has_code {
                // Collision: drain gas, revert, return failure
                frame.evm.gas.drain();
                let gas = frame.evm.gas;
                state.revert_to(frame.checkpoint);
                let result = CallResult::Done { status: Int::ZERO, ret: vec![].into(), gas };
                let gas_final = finalized(&self.call, &tx, &head, effective_gas_price, &result, state);
                let mut result = result;
                result.gas_mut().finalized = gas_final;
                state.apply();
                return Ok(result);
            }
            state.create(
                created,
                Account {
                    value: Int::ZERO,
                    nonce: Int::ONE,
                    code: (Buf::default(), Int::ZERO),
                },
            );
            // EIP-2929: add newly created address to accessed_addresses
            state.warm_acc(&created);
        }
        transfer(&self.call, &mode, state);
        let _ = frame.evm.gas_charge(intrinsic);
        self.callstack.push(frame);

        let mut target: (usize, usize) = (0, 0);
        let mut stipend: i64 = 0;
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
                        // When child succeeds: return all unused gas (including stipend).
                        // When child fails: cap return at gas_sent (stipend is not returned).
                        let unused = (gas.limit - gas.spent).max(0);
                        let return_gas = if status.is_zero() {
                            let gas_sent = gas.limit - stipend;
                            unused.min(gas_sent)
                        } else {
                            unused
                        };
                        this.evm.gas.spent -= return_gas;
                        this.evm.gas.refund += gas.refund;
                        this.evm.apply(state);
                        this.evm.pc += 1;
                        target = (0, 0);
                        stipend = 0;
                        result = None;
                    }
                    CallResult::Created { acc: addr, code, gas } => {
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
                        stipend = 0;
                    }
                }
            }

            match this.evm.step(&this.ctx, &this.call, state)? {
                StepResult::Ok => {
                    continue;
                }
                StepResult::End => {
                    this.evm.apply(state);
                    let is_create = this.call.is_create();
                    let gas = this.evm.gas;
                    result = Some(if is_create {
                        CallResult::Created {
                            acc: this.ctx.this,
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
                    this.evm.apply(state);
                    if is_precompile(&call.to) {
                        // EIP-211: clear return data before new call
                        this.evm.ret.clear();
                        // Precompile runs inline. Replace child-gas reservation with actual used
                        // (avoids OOG when child_gas > remaining); keep access cost.
                        let (ok, out, gas_used) =
                            crate::pre::run(call.to.as_u64(), &call.data.0, call.gas as i64);
                        this.evm.ret = out.clone();
                        this.evm.pending_gas_charge -= call.gas as i64;
                        this.evm.pending_gas_charge += gas_used;
                        let status = if ok { Int::ONE } else { Int::ZERO };
                        let (ret_offset, ret_size) = mode.target().unwrap_or_default();
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

                    let is_create = matches!(mode, CallMode::Create(_) | CallMode::Create2(_));

                    // For CREATE: perform pre-checkpoint checks, then increment nonce.
                    // Per EVM spec, nonce is incremented before the snapshot so it survives
                    // collision reverts, but NOT depth or insufficient-balance failures.
                    if let Some(created) = mode.created() {
                        let creator = call.by;

                        // Depth check before nonce increment
                        if this.ctx.depth + 1 > MAX_CALL_DEPTH {
                            this.evm.apply(state);
                            let _ = this.evm.push(Int::ZERO);
                            this.evm.apply(state);
                            this.evm.ret = vec![];
                            this.evm.pc += 1;
                            target = (0, 0);
                            continue;
                        }

                        // Balance check before nonce increment
                        if !call.eth.is_zero() {
                            let gte =
                                lift(|[a, b]| if a >= b { U256::ONE } else { U256::ZERO });
                            let by0 = state.balance(&creator).unwrap_or_default();
                            if gte([by0, call.eth]).is_zero() {
                                this.evm.apply(state);
                                let _ = this.evm.push(Int::ZERO);
                                this.evm.apply(state);
                                this.evm.ret = vec![];
                                this.evm.pc += 1;
                                target = (0, 0);
                                continue;
                            }
                        }

                        // Increment nonce BEFORE checkpoint so collision-reverts don't undo it
                        state.inc_nonce(&creator, Int::ONE);
                        let _ = created; // suppress unused warning
                    }

                    let checkpoint = state.checkpoint(this.ctx.depth);
                    target = mode.target().unwrap_or_default();
                    stipend = if !call.eth.is_zero()
                        && matches!(mode, CallMode::Call(..) | CallMode::CallCode(..))
                    {
                        2300
                    } else {
                        0
                    };

                    if let Some(created) = mode.created() {
                        let creator = call.by;

                        // Collision check: existing nonce or code at derived address
                        let existing_nonce = state.nonce(&created).unwrap_or(Int::ZERO);
                        let has_code =
                            state.code(&created).is_some_and(|(c, _)| !c.0.is_empty());
                        if !existing_nonce.is_zero() || has_code {
                            state.revert_to(checkpoint);
                            this.evm.apply(state);
                            let _ = this.evm.push(Int::ZERO);
                            this.evm.apply(state);
                            this.evm.ret = vec![];
                            this.evm.pc += 1;
                            target = (0, 0);
                            continue;
                        }

                        // Create account with nonce=1 (EIP-161)
                        state.create(
                            created,
                            Account {
                                value: Int::ZERO,
                                nonce: Int::ONE,
                                code: (Buf::default(), Int::ZERO),
                            },
                        );

                        // EIP-2929: add newly created address to accessed_addresses
                        state.warm_acc(&created);

                        // Value transfer (balance already verified above)
                        if !call.eth.is_zero() {
                            let sub = lift(|[a, b]| a - b);
                            let add = lift(|[a, b]| a + b);
                            let by0 = state.balance(&creator).unwrap_or_default();
                            state.set_value(&creator, sub([by0, call.eth]));
                            let to0 = state.balance(&created).unwrap_or_default();
                            state.set_value(&created, add([to0, call.eth]));
                        }
                    }

                    // EIP-211: clear return data buffer when making a new call
                    this.evm.ret.clear();

                    let frame = prepare(
                        tx.clone(),
                        head.clone(),
                        call.clone(),
                        mode,
                        Some(&this.ctx),
                        state,
                        chain,
                    )
                    .await?;
                    if frame.ctx.depth > MAX_CALL_DEPTH {
                        state.revert_to(checkpoint);
                        this.evm.apply(state);
                        let _ = this.evm.push(Int::ZERO);
                        this.evm.apply(state);
                        this.evm.ret = vec![];
                        this.evm.pc += 1;
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
                            this.evm.apply(state);
                            let _ = this.evm.push(Int::ZERO);
                            this.evm.apply(state);
                            this.evm.ret = vec![];
                            this.evm.pc += 1;
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
                    let is_create = this.call.is_create();
                    result = Some(if is_create {
                        let deploy_cost = CODE_DEPOSIT_GAS * ret.len() as i64;
                        if ret.len() > MAX_CODE_SIZE || this.evm.gas_remaining() < deploy_cost {
                            this.evm.gas.drain();
                            state.revert_to(this.checkpoint);
                            CallResult::Done {
                                status: Int::ZERO,
                                ret: vec![].into(),
                                gas: this.evm.gas,
                            }
                        } else {
                            this.evm.gas.spent += deploy_cost;
                            state.acc_mut(&this.ctx.this).nonce = Int::ONE;
                            CallResult::Created {
                                acc: this.ctx.this,
                                code: ret.into(),
                                gas: this.evm.gas,
                            }
                        }
                    } else {
                        CallResult::Done {
                            status: Int::ONE,
                            ret: ret.into(),
                            gas: this.evm.gas,
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
                    this.evm.apply(state);
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

        let mut result = result.ok_or(Error::Internal("call result missing".into()))?;

        // For top-level CREATE, store the deployed bytecode into the new account.
        if let CallResult::Created { acc: addr, ref code, .. } = result
            && !code.0.is_empty()
        {
            let hash = Int::from(keccak256(code.as_slice()).as_ref());
            state.acc_mut(&addr).code = (code.clone(), hash);
        }

        let gas_final = finalized(&self.call, &tx, &head, effective_gas_price, &result, state);
        result.gas_mut().finalized = gas_final;

        state.apply();
        Ok(result)
    }
}

async fn prepare(
    tx: Tx,
    head: Head,
    call: Call,
    mode: CallMode,
    ctx: Option<&Context>,
    state: &mut impl State,
    chain: &impl Chain,
) -> Result<CallFrame> {
    let is_create = call.is_create();
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
    let evm = Evm::new(head, code.into_vec(), call.gas, tx.gas_price);
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
        evm,
        ctx,
        checkpoint,
    })
}
