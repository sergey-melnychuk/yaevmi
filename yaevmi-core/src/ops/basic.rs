use yaevmi_base::{
    Int,
    math::{ONE, ZERO, lift},
};
use yaevmi_misc::keccak256;

use crate::{
    Call,
    evm::{Context, Evm, EvmResult},
    state::State,
};

pub fn stop(_: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    Ok((0, 0))
}

pub fn add(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a + b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn mul(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a * b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn sub(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a - b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn div(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a / b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

// TODO: 0x05 SDIV

pub fn r#mod(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a % b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

// TODO: 0x07 SMOD

pub fn addmod(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b, m] = evm.popn()?;
    let f = lift(|[a, b, m]| a.add_mod(b, m));
    let r = f([a, b, m]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn mulmod(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b, m] = evm.popn()?;
    let f = lift(|[a, b, m]| a.mul_mod(b, m));
    let r = f([a, b, m]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn exp(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a.pow(b));
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

// TODO: 0x0B SIGNEXTEND

pub fn lt(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| if a < b { ONE } else { ZERO });
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn gt(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| if a > b { ONE } else { ZERO });
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

// TODO: 0x12 SLT
// TODO: 0x13 SGT

pub fn eq(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| if a == b { ONE } else { ZERO });
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn iszero(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [x] = evm.popn()?;
    let f = lift(|[x]| if x.is_zero() { ONE } else { ZERO });
    let r = f([x]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn and(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a & b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn or(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a | b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn xor(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a ^ b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn not(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [x] = evm.popn()?;
    let f = lift(|[x]| !x);
    let r = f([x]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn byte(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [idx, int] = evm.popn()?;
    let byte = int
        .as_ref()
        .get(idx.as_usize())
        .copied()
        .unwrap_or_default();
    evm.push(Int::from(byte))?;
    Ok((gas, 0))
}

pub fn shl(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a << b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn shr(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a >> b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn sar(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [shift, val] = evm.popn()?;
    let f = lift(|[shift, val]| val.arithmetic_shr(shift.saturating_to::<usize>()));
    let r = f([shift, val]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn clz(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    use yaevmi_base::math::U256;
    let gas = 3;
    let [x] = evm.popn()?;
    let f = lift(|[x]| U256::from(x.leading_zeros()));
    let r = f([x]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn hash(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [offset, size] = evm.popn()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());
    let (data, _) = evm.mem_get(offset..offset + size)?;
    let hash = keccak256(data);
    evm.push(hash)?;
    Ok((gas, 0))
}
