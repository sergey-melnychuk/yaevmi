use crate::{
    Call,
    evm::{Context, Evm, EvmResult, EvmYield, HaltReason},
    state::State,
};

const LOG0: u8 = 0xA0;

pub fn log(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<()> {
    let n: usize = (evm.code[evm.pc] - LOG0) as usize;

    let [offset, size] = evm.peek()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());

    let (data, _) = evm.mem_get(offset, size)?;
    let data = data.to_vec();

    // Avoid overflow during gas calculation - check max size first
    let max = (evm.gas.remaining() - (n as i64 + 1) * 375) / 8;
    if size as i64 > max {
        return Err(EvmYield::Halt(HaltReason::OutOfGas));
    }

    let gas = 375 + 375 * n + 8 * size;
    evm.gas.take(gas as i64)?;

    let mut topics = Vec::with_capacity(n);
    for _ in 0..n {
        let [topic] = evm.peek()?;
        topics.push(topic);
    }

    state.log(data.into(), topics);
    evm.pull()?;
    Ok(())
}
