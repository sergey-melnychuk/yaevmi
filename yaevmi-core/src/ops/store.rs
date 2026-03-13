use yaevmi_base::Int;

use crate::{
    Call,
    evm::{Context, Evm, EvmResult, EvmYield, HaltReason},
    state::State,
};

pub fn pop(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    let [_] = evm.peek()?;
    Ok(())
}

pub fn mload(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    let [offset] = evm.peek_usize()?;
    let (data, _) = evm.mem_get(offset, 32)?;
    let int = Int::from(data);
    evm.push(int)?;
    Ok(())
}

pub fn mstore(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    let [offset, value] = evm.peek()?;
    let offset = offset.as_usize();
    evm.mem_put(offset, 32, value.as_ref())?;
    Ok(())
}

pub fn mstore8(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    let [offset, value] = evm.peek()?;
    let (offset, value) = (offset.as_usize(), value.as_u8());
    evm.mem_put(offset, 1, &[value])?;
    Ok(())
}

pub fn sload(evm: &mut Evm, ctx: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(100)?;
    let [key] = evm.peek()?;
    let acc = ctx.this;
    if state.warm_key(&acc, &key) {
        evm.gas_charge(2000)?;
    }
    let Some((val, _)) = state.get(&acc, &key) else {
        return Err(EvmYield::Fetch(crate::evm::Fetch::StateCell(acc, key)));
    };
    evm.push(val)?;
    Ok(())
}

// https://www.evm.codes/?fork=osaka#55
fn sstore_gas(val: Int, cur: Int, org: Int) -> (i64, i64) {
    // static_gas = 0
    // if value == current_value
    //     base_dynamic_gas = 100
    // else if current_value == original_value
    //     if original_value == 0
    //         base_dynamic_gas = 20000
    //     else
    //         base_dynamic_gas = 2900
    // else
    //     base_dynamic_gas = 100
    let g = if val == cur {
        100
    } else if cur == org {
        if org.is_zero() { 20_000 } else { 2_900 }
    } else {
        100
    };

    // if value != current_value
    //     if current_value == original_value
    //         if original_value != 0 and value == 0
    //             gas_refunds += 4800
    //     else
    //         if original_value != 0
    //             if current_value == 0
    //                 gas_refunds -= 4800
    //             else if value == 0
    //                 gas_refunds += 4800
    //         if value == original_value
    //             if original_value == 0
    //                 gas_refunds += 20000 - 100
    //             else
    //                 gas_refunds += 5000 - 2100 - 100
    let mut r = 0;
    if val != cur {
        if cur == org {
            if !org.is_zero() && val.is_zero() {
                r += 4_800;
            }
        } else {
            if !org.is_zero() {
                if cur.is_zero() {
                    r -= 4_800;
                } else if val.is_zero() {
                    r += 4_800;
                }
            }
            if val == org {
                if org.is_zero() {
                    r += 20_000 - 100;
                } else {
                    r += 5_000 - 2_100 - 100;
                }
            }
        }
    }

    (g, r)
}

pub fn sstore(evm: &mut Evm, ctx: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    let [key, val] = evm.peek()?;
    let acc = ctx.this;
    let Some((cur, org)) = state.get(&acc, &key) else {
        return Err(EvmYield::Fetch(crate::evm::Fetch::StateCell(acc, key)));
    };
    let (mut gas, refund) = sstore_gas(val, cur, org);
    if state.warm_key(&acc, &key) {
        gas += 2100;
    }
    evm.gas.refund(refund)?;
    evm.gas_charge(gas)?;
    state.put(&acc, &key, val);
    Ok(())
}

const JUMPDEST: u8 = 0x5B;

pub fn jumpdest(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(1)?;
    Ok(())
}

pub fn jump(evm: &mut Evm, ctx: &Context, call: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(8)?;
    let [dst] = evm.peek()?;
    let dst = dst.as_usize();
    let ok = evm
        .code
        .get(dst)
        .map(|op| op == &JUMPDEST)
        .unwrap_or_default();
    if !ok {
        return Err(EvmYield::Halt(HaltReason::BadJump(dst)));
    }
    evm.pc = dst;
    jumpdest(evm, ctx, call, state)?;
    Ok(())
}

pub fn jumpi(evm: &mut Evm, ctx: &Context, call: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(10)?;
    let [val, dst] = evm.peek()?;
    if val.is_zero() {
        return Ok(());
    }
    let dst = dst.as_usize();
    let ok = evm
        .code
        .get(dst)
        .map(|op| op == &JUMPDEST)
        .unwrap_or_default();
    if !ok {
        return Err(EvmYield::Halt(HaltReason::BadJump(dst)));
    }
    evm.pc = dst;
    jumpdest(evm, ctx, call, state)?;
    Ok(())
}

pub fn pc(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    let pc = evm.pc + 1;
    evm.push(pc.into())?;
    Ok(())
}

pub fn msize(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    let len = evm.memory.len();
    evm.push(len.into())?;
    Ok(())
}

pub fn gas(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    let gas = evm.gas_remaining();
    evm.push((gas as usize).into())?;
    Ok(())
}

pub fn tload(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(100)?;
    let [key] = evm.peek()?;
    let val = state.tget(&key).unwrap_or_default();
    evm.push(val)?;
    Ok(())
}

pub fn tstore(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(100)?;
    let [key, val] = evm.peek()?;
    state.tput(key, val);
    Ok(())
}

pub fn mcopy(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    let [dest_offset, offset, size] = evm.peek_usize()?;
    let (data, pad) = evm.mem_get(offset, size)?;
    let mut copy = vec![0; data.len() + pad];
    copy[..data.len()].copy_from_slice(data);
    evm.mem_put(dest_offset, size, &copy)?;
    Ok(())
}
