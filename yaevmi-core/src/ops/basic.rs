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

pub fn sdiv(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    use yaevmi_base::math::U256;
    let gas = 5;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| {
        if b.is_zero() {
            return U256::ZERO;
        }
        let neg = |x: U256| (!x) + U256::ONE;
        let sign_a = a.bit(255);
        let sign_b = b.bit(255);
        let abs_a = if sign_a { neg(a) } else { a };
        let abs_b = if sign_b { neg(b) } else { b };
        let result = abs_a / abs_b;
        if sign_a != sign_b {
            neg(result)
        } else {
            result
        }
    });
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn r#mod(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| a % b);
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn smod(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    use yaevmi_base::math::U256;
    let gas = 5;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| {
        if b.is_zero() {
            return U256::ZERO;
        }
        let neg = |x: U256| (!x) + U256::ONE;
        let sign_a = a.bit(255);
        let abs_a = if sign_a { neg(a) } else { a };
        let abs_b = if b.bit(255) { neg(b) } else { b };
        let result = abs_a % abs_b;
        if sign_a && !result.is_zero() {
            neg(result)
        } else {
            result
        }
    });
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

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

pub fn signextend(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    use yaevmi_base::math::U256;
    let gas = 5;
    let [b, x] = evm.popn()?;
    let f = lift(|[b, x]| {
        if b >= U256::from(32u32) {
            return x;
        }
        let b = b.saturating_to::<usize>();
        let sign_bit = b * 8 + 7;
        let low_bits = b * 8 + 8; // == (b+1)*8
        let mask = if low_bits == 256 {
            U256::MAX
        } else {
            (U256::ONE << low_bits) - U256::ONE
        };
        if x.bit(sign_bit) { x | !mask } else { x & mask }
    });
    let r = f([b, x]);
    evm.push(r)?;
    Ok((gas, 0))
}

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

pub fn slt(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| {
        // Signed: flip the sign bit so the unsigned order matches signed order
        let key = |x| x ^ (ONE << 255);
        if key(a) < key(b) { ONE } else { ZERO }
    });
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

pub fn sgt(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let gas = 3;
    let [a, b] = evm.popn()?;
    let f = lift(|[a, b]| {
        let key = |x| x ^ (ONE << 255);
        if key(a) > key(b) { ONE } else { ZERO }
    });
    let r = f([a, b]);
    evm.push(r)?;
    Ok((gas, 0))
}

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
