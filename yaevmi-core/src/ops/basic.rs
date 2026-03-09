use crate::{
    Call, Int, Result,
    evm::{Context, Evm, HaltReason, StepResult},
    ops::halt,
    state::State,
};

pub fn stop(_: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> Result<StepResult> {
    Ok(StepResult::End)
}

// TODO: 0x01 ADD
pub fn add(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> Result<StepResult> {
    let gas = 3;
    let Some([_a, _b]) = evm.pop() else {
        return halt(HaltReason::StackUnderflow);
    };
    let sum = Int::ZERO;
    evm.stack.push(sum);
    Ok(StepResult::Ok {
        gas_amount: gas,
        gas_refund: 0,
    })
}

// TODO: 0x02 MUL
// TODO: 0x03 SUB
// TODO: 0x04 DIV
// TODO: 0x05 SDIV
// TODO: 0x06 MOD
// TODO: 0x07 SMOD
// TODO: 0x08 ADDMOD
// TODO: 0x09 MULMOD
// TODO: 0x0A EXP
// TODO: 0x0B SIGNEXTEND
// TODO: 0x10 LT
// TODO: 0x11 GT
// TODO: 0x12 SLT
// TODO: 0x13 SGT
// TODO: 0x14 EQ
// TODO: 0x15 ISZERO
// TODO: 0x16 AND
// TODO: 0x17 OR
// TODO: 0x18 XOR
// TODO: 0x19 NOT
// TODO: 0x1A BYTE
// TODO: 0x1B SHL
// TODO: 0x1C SHR
// TODO: 0x1D SAR
// TODO: 0x20 KECCAK256
