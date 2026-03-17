use yaevmi_base::math::lift;
use yaevmi_base::{Acc, Int};
use yaevmi_misc::keccak256;

use crate::{
    Call,
    aux::{create_address, create2_address, is_precompile},
    evm::{self, CallMode, Context, Evm, EvmResult, EvmYield, Fetch, HaltReason},
    state::State,
};

/// Allocate gas for a child frame per EIP-150 (63/64 rule).
fn sub_call_gas(evm: &Evm) -> u64 {
    let remaining = evm.gas_remaining().max(0) as u64;
    remaining - remaining / 64
}

pub fn create(evm: &mut Evm, ctx: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    if ctx.is_static {
        return Err(EvmYield::Halt(HaltReason::NonStatic));
    }
    evm.gas_charge(32_000)?;
    let [value, offset, size] = evm.peek()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());
    evm::mem_check(offset, size)?;
    let initcode_cost = 2 * ((size as i64 + 31) / 32);
    evm.gas_charge(initcode_cost)?;

    let Some(nonce) = state.nonce(&ctx.this).map(|x| x.as_u64()) else {
        return Err(EvmYield::Fetch(Fetch::Nonce(ctx.this)));
    };
    let address = create_address(&ctx.this, nonce);

    let data = evm.mem_get(offset, size)?;
    let data: Vec<u8> = data.to_vec();
    let gas = sub_call_gas(evm);
    evm.gas_charge(gas as i64)?;

    let call = Call {
        by: ctx.this,
        to: Acc::ZERO,
        gas,
        eth: value,
        data: data.into(),
    };

    Err(EvmYield::Call(call, CallMode::Create(address)))
}

pub fn call(evm: &mut Evm, ctx: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    let [
        gas_arg,
        address,
        value,
        args_offset,
        args_size,
        ret_offset,
        ret_size,
    ] = evm.peek()?;
    let (args_offset, args_size) = (args_offset.as_usize(), args_size.as_usize());
    let (ret_offset, ret_size) = (ret_offset.as_usize(), ret_size.as_usize());
    let address: Acc = address.to();

    if state.acc(&ctx.this).is_none() {
        return Err(EvmYield::Fetch(Fetch::Account(ctx.this)));
    }

    // EIP-2929: warm/cold address access (use pending warm to survive Fetch+reset)
    let access_cost: i64 = if state.is_cold_acc(&address) { 2600 } else { 100 };
    evm.warm_acc(&address);
    evm.gas_charge(access_cost)?;

    // Memory expansion for both args and return regions (BEFORE 63/64 rule)
    evm.mem_expand(args_offset, args_size)?;
    evm.mem_expand(ret_offset, ret_size)?;

    // Value transfer cost
    let has_value = !value.is_zero();
    if ctx.is_static && has_value {
        return Err(EvmYield::Halt(HaltReason::NonStatic));
    }
    if has_value {
        evm.gas_charge(9000)?;
    }

    // New account cost (sending value to dead account per EIP-161).
    // Precompile addresses are always considered "existing" — no new-account cost.
    if !is_precompile(&address) {
        // Must load callee before charging: if not in state, fetch to determine emptiness.
        if state.acc(&address).is_none() {
            return Err(EvmYield::Fetch(Fetch::Account(address)));
        }
        let is_empty = state
            .acc(&address)
            .map(|a| a.value.is_zero() && a.nonce.is_zero() && a.code.0.0.is_empty())
            .unwrap_or(true);
        // EIP-161: only charge when BOTH value>0 AND account is dead (empty).
        if has_value && is_empty {
            evm.gas_charge(25000)?;
        }
    }

    // 63/64 rule: cap the gas arg at available_gas * 63/64
    let available = evm.gas_remaining().max(0) as u64;
    let max_child = available - available / 64;
    let mut gas = gas_arg.as_u64().min(max_child);
    evm.gas_charge(gas as i64)?;

    // Gas stipend: add 2300 to child when sending value
    if has_value {
        gas += 2300;
    }

    let data = evm.mem_get(args_offset, args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas,
        eth: value,
        data: data.to_vec().into(),
    };

    Err(EvmYield::Call(call, CallMode::Call(ret_offset, ret_size)))
}

pub fn callcode(evm: &mut Evm, ctx: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    let [
        gas_arg,
        address,
        value,
        args_offset,
        args_size,
        ret_offset,
        ret_size,
    ] = evm.peek()?;
    let (args_offset, args_size) = (args_offset.as_usize(), args_size.as_usize());
    let (ret_offset, ret_size) = (ret_offset.as_usize(), ret_size.as_usize());
    let address: Acc = address.to();

    if state.acc(&address).is_none() {
        return Err(EvmYield::Fetch(Fetch::Account(address)));
    };

    let access_cost: i64 = if state.is_cold_acc(&address) { 2600 } else { 100 };
    evm.warm_acc(&address);
    evm.gas_charge(access_cost)?;

    evm.mem_expand(args_offset, args_size)?;
    evm.mem_expand(ret_offset, ret_size)?;

    let has_value = !value.is_zero();
    if has_value {
        evm.gas_charge(9000)?;
    }

    let available = evm.gas_remaining().max(0) as u64;
    let max_child = available - available / 64;
    let mut gas = gas_arg.as_u64().min(max_child);
    evm.gas_charge(gas as i64)?;

    if has_value {
        gas += 2300;
    }

    let data = evm.mem_get(args_offset, args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas,
        eth: value,
        data: data.to_vec().into(),
    };

    Err(EvmYield::Call(
        call,
        CallMode::CallCode(ret_offset, ret_size),
    ))
}

pub fn r#return(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    let [offset, size] = evm.peek_usize()?;
    let mem = evm.mem_get(offset, size)?;
    Err(EvmYield::Return(mem.to_vec()))
}

pub fn delegatecall(
    evm: &mut Evm,
    _ctx: &Context,
    call: &Call,
    state: &mut dyn State,
) -> EvmResult<()> {
    let [
        gas_arg,
        address,
        args_offset,
        args_size,
        ret_offset,
        ret_size,
    ] = evm.peek()?;
    let (args_offset, args_size) = (args_offset.as_usize(), args_size.as_usize());
    let (ret_offset, ret_size) = (ret_offset.as_usize(), ret_size.as_usize());
    let address: Acc = address.to();

    if state.acc(&address).is_none() {
        return Err(EvmYield::Fetch(Fetch::Account(address)));
    };

    let access_cost: i64 = if state.is_cold_acc(&address) { 2600 } else { 100 };
    evm.warm_acc(&address);
    evm.gas_charge(access_cost)?;

    evm.mem_expand(args_offset, args_size)?;
    evm.mem_expand(ret_offset, ret_size)?;

    let available = evm.gas_remaining().max(0) as u64;
    let max_child = available - available / 64;
    let gas = gas_arg.as_u64().min(max_child);
    evm.gas_charge(gas as i64)?;

    let data = evm.mem_get(args_offset, args_size)?;

    // DELEGATECALL preserves msg.sender (caller) from the parent frame
    let inner_call = Call {
        by: call.by,
        to: address,
        gas,
        eth: Int::ZERO,
        data: data.to_vec().into(),
    };

    Err(EvmYield::Call(
        inner_call,
        CallMode::Delegate(ret_offset, ret_size),
    ))
}

pub fn create2(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    if ctx.is_static {
        return Err(EvmYield::Halt(HaltReason::NonStatic));
    }
    evm.gas_charge(32_000)?;
    let [value, offset, size, salt] = evm.peek()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());
    evm::mem_check(offset, size)?;
    // EIP-3860 initcode word cost (2) + CREATE2 hash word cost (6) = 8 per word
    let word_cost = 8 * ((size as i64 + 31) / 32);
    evm.gas_charge(word_cost)?;

    let data = evm.mem_get(offset, size)?;
    let data: Vec<u8> = data.to_vec();
    let init_code_hash = Int::from(keccak256(&data).as_ref());
    let address = create2_address(&ctx.this, &salt, &init_code_hash);

    let gas = sub_call_gas(evm);
    evm.gas_charge(gas as i64)?;

    let call = Call {
        by: ctx.this,
        to: Acc::ZERO,
        gas,
        eth: value,
        data: data.into(),
    };

    Err(EvmYield::Call(call, CallMode::Create2(address)))
}

pub fn staticcall(evm: &mut Evm, ctx: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    let [
        gas_arg,
        address,
        args_offset,
        args_size,
        ret_offset,
        ret_size,
    ] = evm.peek()?;
    let (args_offset, args_size) = (args_offset.as_usize(), args_size.as_usize());
    let (ret_offset, ret_size) = (ret_offset.as_usize(), ret_size.as_usize());
    let address: Acc = address.to();
    let is_precompile = is_precompile(&address);

    // EIP-2929: warm/cold address access (applies to precompiles too)
    let access_cost: i64 = if state.is_cold_acc(&address) { 2600 } else { 100 };
    evm.warm_acc(&address);
    evm.gas_charge(access_cost)?;

    evm.mem_expand(args_offset, args_size)?;
    evm.mem_expand(ret_offset, ret_size)?;

    let available = evm.gas_remaining().max(0) as u64;
    let max_child = available - available / 64;
    let gas = gas_arg.as_u64().min(max_child);

    if !is_precompile && state.acc(&address).is_none() {
        return Err(EvmYield::Fetch(Fetch::Account(address)));
    };

    evm.gas_charge(gas as i64)?;

    let data = evm.mem_get(args_offset, args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas,
        eth: Int::ZERO,
        data: data.to_vec().into(),
    };

    Err(EvmYield::Call(call, CallMode::Static(ret_offset, ret_size)))
}

pub fn revert(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    let [offset, size] = evm.peek_usize()?;
    let mem = evm.mem_get(offset, size)?;
    Err(EvmYield::Revert(mem.to_vec()))
}

pub fn selfdestruct(
    evm: &mut Evm,
    ctx: &Context,
    _: &Call,
    state: &mut dyn State,
) -> EvmResult<()> {
    evm.gas_charge(5_000)?;

    // EIP-214: SELFDESTRUCT in static context is an exceptional halt
    if ctx.is_static {
        return Err(EvmYield::Halt(HaltReason::NonStatic));
    }

    let [beneficiary] = evm.peek()?;
    let beneficiary: Acc = beneficiary.to();

    // EIP-2929: cold address surcharge for beneficiary (no warm cost — covered by 5000 base)
    if state.is_cold_acc(&beneficiary) {
        evm.gas_charge(2600)?;
    }
    evm.warm_acc(&beneficiary);

    let balance = state.balance(&ctx.this).unwrap_or(Int::ZERO);
    if !balance.is_zero() && beneficiary != ctx.this {
        // EIP-161: creating empty account costs 25000 when transferring value
        let is_empty = state
            .acc(&beneficiary)
            .map(|a| a.value.is_zero() && a.nonce.is_zero() && a.code.0.0.is_empty())
            .unwrap_or(true);
        if is_empty {
            evm.gas_charge(25_000)?;
        }
        let add = lift(|[a, b]| a + b);
        let to_bal = state.balance(&beneficiary).unwrap_or(Int::ZERO);
        state.set_value(&beneficiary, add([to_bal, balance]));
        state.set_value(&ctx.this, Int::ZERO);
    }

    // EIP-6780 (Cancun): only destroy if contract was created in same transaction
    if state.created().contains(&ctx.this) {
        state.destroy(&ctx.this);
    }
    Err(EvmYield::Return(vec![]))
}
