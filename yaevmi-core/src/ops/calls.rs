use yaevmi_base::{Acc, Int};
use yaevmi_misc::keccak256;

use crate::{
    Call,
    aux::{create_address, create2_address},
    evm::{self, CallMode, Context, Evm, EvmResult, EvmYield, Fetch},
    state::State,
};

/// Allocate gas for a child frame per EIP-150 (63/64 rule).
fn sub_call_gas(evm: &Evm) -> u64 {
    let remaining = evm.gas_remaining().max(0) as u64;
    remaining - remaining / 64
}

pub fn create(evm: &mut Evm, ctx: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(32_000)?;
    let [value, offset, size] = evm.peek()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());
    evm::mem_check(offset, size)?;
    let initcode_cost = 2 * (size as i64 + 31) / 32;
    evm.gas_charge(initcode_cost)?;

    let Some(nonce) = state.nonce(&ctx.this).map(|x| x.as_u64()) else {
        return Err(EvmYield::Fetch(Fetch::Nonce(ctx.this)));
    };
    let address = create_address(&ctx.this, nonce);

    let (data, _pad) = evm.mem_get(offset, size)?;
    let data: Vec<u8> = data.to_vec();
    let gas = sub_call_gas(evm);
    evm.gas_charge(gas as i64)?;

    let call = Call {
        by: ctx.this,
        to: Acc::ZERO,
        gas,
        eth: value,
        data: data.into(),
        auth: vec![],
        nonce: None,
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
    };

    // EIP-2929: warm/cold address access
    let access_cost: i64 = if state.is_cold_acc(&address) {
        evm.warm_acc(&address);
        2600
    } else {
        100
    };
    evm.gas_charge(access_cost)?;

    // Memory expansion for both args and return regions
    evm::mem_check(args_offset, args_size)?;
    evm::mem_check(ret_offset, ret_size)?;

    // Value transfer cost
    let has_value = !value.is_zero();
    if has_value {
        evm.gas_charge(9000)?;
    }

    // New account cost (sending value to dead account per EIP-161)
    let is_empty = state
        .acc(&address)
        .map(|a| a.value.is_zero() && a.nonce.is_zero() && a.code.0.0.is_empty())
        .unwrap_or(true);
    if has_value && is_empty {
        evm.gas_charge(25000)?;
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

    let (data, _pad) = evm.mem_get(args_offset, args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas,
        eth: value,
        data: data.to_vec().into(),
        auth: vec![],
        nonce: None,
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

    let access_cost: i64 = if state.is_cold_acc(&address) {
        evm.warm_acc(&address);
        2600
    } else {
        100
    };
    evm.gas_charge(access_cost)?;

    evm::mem_check(args_offset, args_size)?;
    evm::mem_check(ret_offset, ret_size)?;

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

    let (data, _pad) = evm.mem_get(args_offset, args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas,
        eth: value,
        data: data.to_vec().into(),
        auth: vec![],
        nonce: None,
    };

    Err(EvmYield::Call(
        call,
        CallMode::CallCode(ret_offset, ret_size),
    ))
}

pub fn r#return(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    let [offset, size] = evm.peek_usize()?;
    let (mem, pad) = evm.mem_get(offset, size)?;
    let mut ret = vec![0; mem.len() + pad];
    ret[..mem.len()].copy_from_slice(mem);
    Err(EvmYield::Return(ret))
}

pub fn delegatecall(
    evm: &mut Evm,
    ctx: &Context,
    _: &Call,
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

    let access_cost: i64 = if state.is_cold_acc(&address) {
        evm.warm_acc(&address);
        2600
    } else {
        100
    };
    evm.gas_charge(access_cost)?;

    evm::mem_check(args_offset, args_size)?;
    evm::mem_check(ret_offset, ret_size)?;

    let available = evm.gas_remaining().max(0) as u64;
    let max_child = available - available / 64;
    let gas = gas_arg.as_u64().min(max_child);
    evm.gas_charge(gas as i64)?;

    let (data, _pad) = evm.mem_get(args_offset, args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas,
        eth: Int::ZERO,
        data: data.to_vec().into(),
        auth: vec![],
        nonce: None,
    };

    Err(EvmYield::Call(
        call,
        CallMode::Delegate(ret_offset, ret_size),
    ))
}

pub fn create2(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(32_000)?;
    let [value, offset, size, salt] = evm.peek()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());
    evm::mem_check(offset, size)?;
    // EIP-3860 initcode word cost (2) + CREATE2 hash word cost (6) = 8 per word
    let word_cost = 8 * (size as i64 + 31) / 32;
    evm.gas_charge(word_cost)?;

    let (data, _pad) = evm.mem_get(offset, size)?;
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
        auth: vec![],
        nonce: None,
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

    if state.acc(&address).is_none() {
        return Err(EvmYield::Fetch(Fetch::Account(address)));
    };

    let access_cost: i64 = if state.is_cold_acc(&address) {
        evm.warm_acc(&address);
        2600
    } else {
        100
    };
    evm.gas_charge(access_cost)?;

    evm::mem_check(args_offset, args_size)?;
    evm::mem_check(ret_offset, ret_size)?;

    let available = evm.gas_remaining().max(0) as u64;
    let max_child = available - available / 64;
    let gas = gas_arg.as_u64().min(max_child);
    evm.gas_charge(gas as i64)?;

    let (data, _pad) = evm.mem_get(args_offset, args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas,
        eth: Int::ZERO,
        data: data.to_vec().into(),
        auth: vec![],
        nonce: None,
    };

    Err(EvmYield::Call(call, CallMode::Static(ret_offset, ret_size)))
}

pub fn revert(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    let [offset, size] = evm.peek_usize()?;
    let (mem, pad) = evm.mem_get(offset, size)?;
    let mut ret = vec![0; mem.len() + pad];
    ret[..mem.len()].copy_from_slice(mem);
    Err(EvmYield::Revert(ret))
}

// TODO: 0xFF SELFDESTRUCT
pub fn selfdestruct(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(5_000)?;
    // TODO: transfer value, mark deleted
    Ok(())
}
