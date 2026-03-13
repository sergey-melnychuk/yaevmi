use yaevmi_base::Acc;

use crate::{
    Call,
    evm::{Context, Evm, EvmResult, EvmYield, Fetch, HaltReason, mem_check},
    state::State,
};

pub fn address(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    let r = ctx.this.to();
    evm.push(r)?;
    Ok(())
}

pub fn balance(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(100)?;
    let [acc] = evm.peek()?;
    let acc: Acc = acc.to();
    let Some(balance) = state.balance(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Balance(acc)));
    };

    if state.is_cold_acc(&acc) {
        evm.warm_acc(&acc);
        evm.gas_charge(2_500)?;
    }
    evm.push(balance)?;
    Ok(())
}

pub fn origin(evm: &mut Evm, ctx: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(ctx.origin.to())?;
    Ok(())
}

pub fn caller(evm: &mut Evm, _: &Context, call: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(call.by.to())?;
    Ok(())
}

pub fn callvalue(evm: &mut Evm, _: &Context, call: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(call.eth)?;
    Ok(())
}

pub fn calldataload(evm: &mut Evm, _: &Context, call: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    let [offset] = evm.peek_usize()?;
    let mut word = [0u8; 32];
    if offset < call.data.0.len() {
        let copy = (call.data.0.len() - offset).min(32);
        word[..copy].copy_from_slice(&call.data.0[offset..offset + copy]);
    }
    evm.push(yaevmi_base::Int::from(&word[..]))?;
    Ok(())
}

pub fn calldatasize(evm: &mut Evm, _: &Context, call: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(call.data.0.len().into())?;
    Ok(())
}

pub fn calldatacopy(evm: &mut Evm, _: &Context, call: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    let [dest_offset, offset, size] = evm.peek_usize()?;
    mem_check(dest_offset, size)?;
    evm.gas_charge(3 * size.div_ceil(32) as i64)?;
    let lo = offset.min(call.data.0.len());
    let hi = offset.saturating_add(size).min(call.data.0.len());
    let len = (lo..hi).len();
    if len > 0 {
        let data = &call.data.0[lo..hi];
        evm.mem_put(dest_offset, len, data)?;
    }
    if len < size {
        let pad = size - (lo..hi).len();
        let data = vec![0; pad];
        evm.mem_put(dest_offset + len, size - len, &data)?;
    }
    Ok(())
}

pub fn codesize(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    let size = evm.code.len();
    evm.push(size.into())?;
    Ok(())
}

pub fn codecopy(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    let [dest_offset, offset, size] = evm.peek_usize()?;
    mem_check(dest_offset, size)?;
    evm.gas_charge(3 * size.div_ceil(32) as i64)?;
    let lo = offset.min(evm.code.len());
    let hi = offset.saturating_add(size).min(evm.code.len());
    let len = (lo..hi).len();
    let mut ret = vec![0u8; size];
    ret[0..len].copy_from_slice(&evm.code[lo..hi]);
    evm.mem_put(dest_offset, size, &ret)?;
    Ok(())
}

pub fn gasprice(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.gas_price)?;
    Ok(())
}

pub fn extcodesize(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(100)?;
    let [acc] = evm.peek()?;
    let acc: Acc = acc.to();
    let Some((code, _)) = state.code(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Code(acc)));
    };
    if state.is_cold_acc(&acc) {
        evm.warm_acc(&acc);
        evm.gas_charge(2_500)?;
    }
    evm.push(code.0.len().into())?;
    Ok(())
}

pub fn extcodecopy(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(100)?;
    let [acc, dest_offset, offset, size] = evm.peek()?;
    let acc: Acc = acc.to();
    let (dest_offset, offset, size) = (dest_offset.as_usize(), offset.as_usize(), size.as_usize());

    let Some((code, _)) = state.code(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Code(acc)));
    };
    if state.is_cold_acc(&acc) {
        evm.warm_acc(&acc);
        evm.gas_charge(2_500)?;
    }
    evm.gas_charge(3 * size.div_ceil(32) as i64)?;

    let lo = offset.min(code.0.len());
    let hi = offset.saturating_add(size).min(code.0.len());
    let len = (lo..hi).len();
    if len > 0 {
        let data = &code.0[lo..hi];
        evm.mem_put(dest_offset, len, data)?;
    }
    if len < size {
        let pad = size - (lo..hi).len();
        let data = vec![0; pad];
        evm.mem_put(dest_offset + len, size - len, &data)?;
    }
    Ok(())
}

pub fn returndatasize(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.ret.len().into())?;
    Ok(())
}

pub fn returndatacopy(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    let [dest_offset, offset, size] = evm.peek_usize()?;
    mem_check(offset, size)?;
    mem_check(dest_offset, size)?;
    evm.gas_charge(3 * size.div_ceil(32) as i64)?;

    // EVM spec: halt if copy range exceeds return data buffer
    if offset.saturating_add(size) > evm.ret.len() {
        return Err(EvmYield::Halt(HaltReason::BadCopyRange));
    }

    if size > 0 {
        let data = evm.ret[offset..offset + size].to_vec();
        evm.mem_put(dest_offset, size, &data)?;
    }
    Ok(())
}

pub fn extcodehash(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(100)?;
    let [acc] = evm.peek()?;
    let acc: Acc = acc.to();
    let Some((_, hash)) = state.code(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Code(acc)));
    };
    if state.is_cold_acc(&acc) {
        evm.warm_acc(&acc);
        evm.gas_charge(2_500)?;
    }
    evm.push(hash)?;
    Ok(())
}

pub fn blockhash(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(20)?;
    let [number] = evm.peek()?;
    let number = number.as_u64();
    let Some(head) = state.head(number) else {
        return Err(EvmYield::Fetch(Fetch::BlockHash(number)));
    };
    evm.push(head.hash)?;
    Ok(())
}

pub fn coinbase(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.coinbase.to())?;
    Ok(())
}

pub fn timestamp(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.timestamp)?;
    Ok(())
}

pub fn number(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.number.into())?;
    Ok(())
}

pub fn prevrandao(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.prevrandao)?;
    Ok(())
}

pub fn gaslimit(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.gas_limit)?;
    Ok(())
}

pub fn chainid(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.chain_id.into())?;
    Ok(())
}

pub fn selfbalance(evm: &mut Evm, ctx: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(5)?;
    let acc: Acc = ctx.this;
    let Some(balance) = state.balance(&acc) else {
        return Err(EvmYield::Fetch(Fetch::Balance(acc)));
    };
    evm.push(balance)?;
    Ok(())
}

pub fn basefee(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.base_fee)?;
    Ok(())
}

pub fn blobhash(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(3)?;
    evm.push(evm.head.blobhash)?;
    Ok(())
}

pub fn blobbasefee(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> EvmResult<()> {
    evm.gas_charge(2)?;
    evm.push(evm.head.blob_base_fee)?;
    Ok(())
}
