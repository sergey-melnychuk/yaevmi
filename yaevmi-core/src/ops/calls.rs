use yaevmi_base::{Acc, Int};

use crate::{
    Call,
    evm::{CallMode, Context, Evm, EvmResult, EvmYield},
    state::State,
};

// TODO: 0xF0 CREATE
pub fn create(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let [value, offset, size] = evm.popn()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());
    let (data, _pad) = evm.mem_get(offset..offset + size)?;

    let call = Call {
        by: ctx.this,
        to: Acc::ZERO,
        gas: 0, // TODO
        eth: value,
        data: data.to_vec(),
        auth: vec![],
        nonce: None,
    };

    // address = keccak256(rlp([sender_address,sender_nonce]))[12:]
    let address = Acc::ZERO;
    Err(EvmYield::Call(call, CallMode::Create(address)))
}

// TODO: 0xF1 CALL
pub fn call(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let [
        gas,
        address,
        value,
        args_offset,
        args_size,
        ret_offset,
        ret_size,
    ] = evm.popn()?;
    let (args_offset, args_size) = (args_offset.as_usize(), args_size.as_usize());
    let (ret_offset, ret_size) = (ret_offset.as_usize(), ret_size.as_usize());
    let address: Acc = address.to();

    let (data, _pad) = evm.mem_get(args_offset..args_offset + args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas: gas.as_u64(),
        eth: value,
        data: data.to_vec(),
        auth: vec![],
        nonce: None,
    };

    Err(EvmYield::Call(
        call,
        CallMode::Call(ret_offset..ret_offset + ret_size),
    ))
}

// TODO: 0xF2 CALLCODE
pub fn callcode(
    evm: &mut Evm,
    ctx: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let [
        gas,
        address,
        value,
        args_offset,
        args_size,
        ret_offset,
        ret_size,
    ] = evm.popn()?;
    let (args_offset, args_size) = (args_offset.as_usize(), args_size.as_usize());
    let (ret_offset, ret_size) = (ret_offset.as_usize(), ret_size.as_usize());
    let address: Acc = address.to();

    let (data, _pad) = evm.mem_get(args_offset..args_offset + args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas: gas.as_u64(),
        eth: value,
        data: data.to_vec(),
        auth: vec![],
        nonce: None,
    };

    Err(EvmYield::Call(
        call,
        CallMode::CallCode(ret_offset..ret_offset + ret_size),
    ))
}

pub fn r#return(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let [offset, size] = evm.popn_usize()?;
    let (mem, pad) = evm.mem_get(offset..offset + size)?;
    let mut ret = vec![0; mem.len() + pad];
    ret[..mem.len()].copy_from_slice(mem);
    Err(EvmYield::Return(ret))
}

// TODO: 0xF4 DELEGATECALL
pub fn delegatecall(
    evm: &mut Evm,
    ctx: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let [
        gas,
        address,
        value,
        args_offset,
        args_size,
        ret_offset,
        ret_size,
    ] = evm.popn()?;
    let (args_offset, args_size) = (args_offset.as_usize(), args_size.as_usize());
    let (ret_offset, ret_size) = (ret_offset.as_usize(), ret_size.as_usize());
    let address: Acc = address.to();

    let (data, _pad) = evm.mem_get(args_offset..args_offset + args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas: gas.as_u64(),
        eth: value,
        data: data.to_vec(),
        auth: vec![],
        nonce: None,
    };

    Err(EvmYield::Call(
        call,
        CallMode::Delegate(ret_offset..ret_offset + ret_size),
    ))
}

// TODO: 0xF5 CREATE2
pub fn create2(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let [value, offset, size, _salt] = evm.popn()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());
    let (data, _pad) = evm.mem_get(offset..offset + size)?;

    let call = Call {
        by: ctx.this,
        to: Acc::ZERO,
        gas: 0, // TODO
        eth: value,
        data: data.to_vec(),
        auth: vec![],
        nonce: None,
    };

    // initialisation_code = memory[offset:offset+size]
    // address = keccak256(0xff + sender_address + salt + keccak256(initialisation_code))[12:]
    let address = Acc::ZERO;
    Err(EvmYield::Call(call, CallMode::Create2(address)))
}

// TODO: 0xFA STATICCALL
pub fn staticcall(
    evm: &mut Evm,
    ctx: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let [gas, address, args_offset, args_size, ret_offset, ret_size] = evm.popn()?;
    let (args_offset, args_size) = (args_offset.as_usize(), args_size.as_usize());
    let (ret_offset, ret_size) = (ret_offset.as_usize(), ret_size.as_usize());
    let address: Acc = address.to();

    let (data, _pad) = evm.mem_get(args_offset..args_offset + args_size)?;

    let call = Call {
        by: ctx.this,
        to: address,
        gas: gas.as_u64(),
        eth: Int::ZERO,
        data: data.to_vec(),
        auth: vec![],
        nonce: None,
    };

    Err(EvmYield::Call(
        call,
        CallMode::Call(ret_offset..ret_offset + ret_size),
    ))
}

pub fn revert(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let [offset, size] = evm.popn_usize()?;
    let (mem, pad) = evm.mem_get(offset..offset + size)?;
    let mut ret = vec![0; mem.len() + pad];
    ret[..mem.len()].copy_from_slice(mem);
    Err(EvmYield::Revert(ret))
}

// TODO: 0xFF SELFDESTRUCT
pub fn selfdestruct(
    _evm: &mut Evm,
    _: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    // TODO: transfer value, mark deleted
    Ok((0, 0))
}
