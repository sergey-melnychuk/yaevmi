use crate::{
    Call,
    evm::{Context, Evm, EvmResult, HaltReason},
    state::State,
};

pub fn pop(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let [_] = evm.popn()?;
    Ok((3, 0))
}

// TODO: 0x51 MLOAD
// TODO: 0x52 MSTORE
// TODO: 0x53 MSTORE8
// TODO: 0x54 SLOAD
// TODO: 0x55 SSTORE

const JUMPDEST: u8 = 0x5B;

pub fn jump(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 5;
    let [dst] = evm.popn()?;
    let dst = dst.as_usize();
    let ok = evm
        .code
        .get(dst)
        .map(|op| op == &JUMPDEST)
        .unwrap_or_default();
    if !ok {
        return Err(HaltReason::BadJump(dst));
    }
    evm.pc = dst;
    Ok((gas, 0))
}

pub fn jumpi(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 5;
    let [val, dst] = evm.popn()?;
    if val.is_zero() {
        return Ok((gas, 0));
    }
    let dst = dst.as_usize();
    let ok = evm
        .code
        .get(dst)
        .map(|op| op == &JUMPDEST)
        .unwrap_or_default();
    if !ok {
        return Err(HaltReason::BadJump(dst));
    }
    evm.pc = dst;
    Ok((gas, 0))
}

pub fn pc(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let pc = evm.pc + 1;
    evm.push(pc.into())?;
    Ok((3, 0))
}

pub fn msize(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let len = evm.memory.len();
    evm.push(len.into())?;
    Ok((3, 0))
}

pub fn gas(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = evm.gas.limit - evm.gas.spent + evm.gas.refund;
    evm.push((gas as usize).into())?;
    Ok((3, 0))
}

// TODO: 0x5C TLOAD
// TODO: 0x5D TSTORE
// TODO: 0x5E MCOPY
