use crate::{
    Call,
    evm::{Context, Evm, EvmResult},
    state::State,
};

const LOG0: u8 = 0xA0;

pub fn log(evm: &mut Evm, _: &Context, _: &Call, state: &mut dyn State) -> EvmResult<(i64, i64)> {
    let n: usize = (evm.code[evm.pc] - LOG0) as usize;

    let [offset, size] = evm.popn()?;
    let (offset, size) = (offset.as_usize(), size.as_usize());

    let mut topics = Vec::with_capacity(n);
    for _ in 0..n {
        let [topic] = evm.popn()?;
        topics.push(topic);
    }
    let (data, _) = evm.mem_get(offset..offset + size)?;

    state.log(data.to_vec(), topics);

    let gas = 375 + 375 * n + 8 * size;
    Ok((gas as i64, 0))
}
