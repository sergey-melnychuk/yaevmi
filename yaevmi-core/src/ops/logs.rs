use crate::{
    Call,
    evm::{Context, Evm, EvmResult, EvmYield, HaltReason},
    state::State,
};

const LOG0: u8 = 0xA0;

pub fn log(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    let n: usize = (evm.code[evm.pc] - LOG0) as usize;
    let total = 2 + n;
    if evm.stack.len() < total {
        return Err(EvmYield::Halt(HaltReason::StackUnderflow));
    }

    let [offset, size] = evm.peek()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());

    // Avoid overflow during gas calculation - check max size first
    let max = (evm.gas_remaining() - (n as i64 + 1) * 375) / 8;
    if size as i64 > max {
        return Err(EvmYield::Halt(HaltReason::OutOfGas));
    }

    let gas = 375 + 375 * n + 8 * size;
    evm.gas_charge(gas as i64)?;

    let (data, _) = evm.mem_get(offset, size)?;
    let data = data.to_vec();

    let topics = evm.stack.iter().rev().skip(2).take(n).cloned().collect();

    evm.pending_stack_pops = total;
    state.log(data.into(), topics);
    Ok(())
}
