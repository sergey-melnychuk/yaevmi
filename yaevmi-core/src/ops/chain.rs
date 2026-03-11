use yaevmi_base::Acc;

use crate::{
    Call,
    evm::{Context, Evm, EvmResult, EvmYield, Fetch},
    state::State,
};

pub fn address(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let r = ctx.this.to();
    evm.push(r)?;
    Ok((2, 0))
}

pub fn balance(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    state: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let [acc] = evm.popn()?;
    let acc: Acc = acc.to();
    let Some(balance) = state.balance(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Balance(acc)));
    };
    evm.push(balance)?;
    Ok((100, 0))
}

pub fn origin(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(ctx.origin.to())?;
    Ok((2, 0))
}

pub fn caller(evm: &mut Evm, _: &Context, call: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(call.by.to())?;
    Ok((2, 0))
}

pub fn callvalue(
    evm: &mut Evm,
    _: &Context,
    call: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    evm.push(call.eth)?;
    Ok((2, 0))
}

pub fn calldataload(
    evm: &mut Evm,
    _: &Context,
    call: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let [offset] = evm.popn_usize()?;
    let byte = call.data.get(offset).copied().unwrap_or_default();
    evm.push(byte.into())?;
    Ok((2, 0))
}

pub fn calldatasize(
    evm: &mut Evm,
    _: &Context,
    call: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    evm.push(call.data.len().into())?;
    Ok((2, 0))
}

pub fn calldatacopy(
    evm: &mut Evm,
    _: &Context,
    call: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let mut gas = 3;
    let [dest_offset, offset, size] = evm.popn_usize()?;
    let lo = offset.min(call.data.len());
    let hi = (offset + size).min(call.data.len());
    let len = (lo..hi).len();
    if len > 0 {
        let data = &call.data[lo..hi];
        let mem_exp_cost = evm.mem_put(dest_offset..dest_offset + len, data)?;
        gas += mem_exp_cost;
    }
    if len < size {
        let pad = size - (lo..hi).len();
        let data = vec![0; pad];
        let mem_exp_cost = evm.mem_put(dest_offset + len..dest_offset + size, &data)?;
        gas += mem_exp_cost;
    }
    Ok((gas, 0))
}

pub fn codesize(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let size = evm.code.len();
    evm.push(size.into())?;
    Ok((3, 0))
}

pub fn codecopy(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    let mut gas = 3;
    let [dest_offset, offset, size] = evm.popn_usize()?;
    let lo = offset.min(evm.code.len());
    let hi = (offset + size).min(evm.code.len());
    let len = (lo..hi).len();
    if len > 0 {
        let data = evm.code[lo..hi].to_vec();
        let mem_exp_cost = evm.mem_put(dest_offset..dest_offset + len, &data)?;
        gas += mem_exp_cost;
    }
    if len < size {
        let pad = size - (lo..hi).len();
        let data = vec![0; pad];
        let mem_exp_cost = evm.mem_put(dest_offset + len..dest_offset + size, &data)?;
        gas += mem_exp_cost;
    }
    Ok((gas, 0))
}

pub fn gasprice(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.gas_price)?;
    Ok((3, 0))
}

pub fn extcodesize(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    state: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let [acc] = evm.popn()?;
    let acc: Acc = acc.to();
    let Some((code, _)) = state.code(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Code(acc)));
    };
    evm.push(code.len().into())?;
    Ok((3, 0))
}

pub fn extcodecopy(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    state: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let mut gas = 3;
    let [acc, dest_offset, offset, size] = evm.popn()?;
    let acc: Acc = acc.to();
    let (dest_offset, offset, size) = (dest_offset.as_usize(), offset.as_usize(), size.as_usize());

    let Some((code, _)) = state.code(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Code(acc)));
    };

    let lo = offset.min(code.len());
    let hi = (offset + size).min(code.len());
    let len = (lo..hi).len();
    if len > 0 {
        let data = &code[lo..hi];
        let mem_exp_cost = evm.mem_put(dest_offset..dest_offset + len, data)?;
        gas += mem_exp_cost;
    }
    if len < size {
        let pad = size - (lo..hi).len();
        let data = vec![0; pad];
        let mem_exp_cost = evm.mem_put(dest_offset + len..dest_offset + size, &data)?;
        gas += mem_exp_cost;
    }
    Ok((gas, 0))
}

pub fn returndatasize(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let gas = 3;
    evm.push(evm.ret.len().into())?;
    Ok((gas, 0))
}

pub fn returndatacopy(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let mut gas = 3;
    let [dest_offset, offset, size] = evm.popn_usize()?;

    let lo = offset.min(evm.ret.len());
    let hi = (offset + size).min(evm.ret.len());
    let len = (lo..hi).len();
    if len > 0 {
        let data = evm.ret[lo..hi].to_vec();
        let mem_exp_cost = evm.mem_put(dest_offset..dest_offset + len, &data)?;
        gas += mem_exp_cost;
    }
    if len < size {
        let pad = size - (lo..hi).len();
        let data = vec![0; pad];
        let mem_exp_cost = evm.mem_put(dest_offset + len..dest_offset + size, &data)?;
        gas += mem_exp_cost;
    }
    Ok((gas, 0))
}

pub fn extcodehash(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    state: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let [acc] = evm.popn()?;
    let acc: Acc = acc.to();
    let Some((_, hash)) = state.code(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Code(acc)));
    };
    evm.push(hash)?;
    Ok((3, 0))
}

pub fn blockhash(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    state: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let [number] = evm.popn()?;
    let number = number.as_u64();
    let Some(head) = state.head(number) else {
        return Err(EvmYield::Fetch(Fetch::BlockHash(number)));
    };
    evm.push(head.hash)?;
    Ok((3, 0))
}

pub fn coinbase(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.coinbase)?;
    Ok((3, 0))
}

pub fn timestamp(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.timestamp)?;
    Ok((3, 0))
}

pub fn number(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.number.into())?;
    Ok((3, 0))
}

pub fn prevrandao(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.prevrandao)?;
    Ok((3, 0))
}

pub fn gaslimit(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.gas_limit)?;
    Ok((3, 0))
}

pub fn chainid(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.chain_id)?;
    Ok((3, 0))
}

pub fn selfbalance(
    evm: &mut Evm,
    ctx: &Context,
    _: &Call,
    state: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    let acc: Acc = ctx.this;
    let Some(balance) = state.balance(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Balance(acc)));
    };
    evm.push(balance)?;
    Ok((5, 0))
}

pub fn basefee(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.base_fee)?;
    Ok((3, 0))
}

pub fn blobhash(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.blobhash)?;
    Ok((3, 0))
}

pub fn blobbasefee(
    evm: &mut Evm,
    _: &Context,
    _: &Call,
    _: &mut dyn State,
) -> EvmResult<(i64, i64)> {
    evm.push(evm.head.blob_base_fee)?;
    Ok((3, 0))
}
