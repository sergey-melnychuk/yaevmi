use crate::{
    Call, Int, Result,
    evm::{Context, Evm, HaltReason, StepResult},
    ops::{halt, ok},
    state::State,
};

pub fn push(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> Result<StepResult> {
    let gas = 3;
    let op = evm.code[evm.pc];
    let len = len(op);
    let int = if len == 0 {
        Int::ZERO
    } else {
        let lo = evm.pc + 1;
        let hi = evm.pc + 1 + len;
        if hi > evm.code.len() {
            return halt(HaltReason::BadOpcode(op));
        }
        Int::from(&evm.code[lo..hi])
    };
    evm.stack.push(int);
    evm.pc += len + 1;
    ok(gas)
}

pub fn dup(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> Result<StepResult> {
    let gas = 3;
    let op = evm.code[evm.pc];
    let n = idx(op) - 1;
    let Some(int) = evm.stack.iter().rev().nth(n).copied() else {
        return halt(HaltReason::StackUnderflow);
    };
    evm.stack.push(int);
    evm.pc += 1;
    ok(gas)
}

pub fn swap(evm: &mut Evm, _: &Context, _: &Call, _: &mut dyn State) -> Result<StepResult> {
    let gas = 3;
    let op = evm.code[evm.pc];
    let n = idx(op); // SWAP{k}: swap top with (k+1)th, distance = k = idx(op)
    if evm.stack.len() <= n {
        return halt(HaltReason::StackUnderflow);
    }
    let i = evm.stack.len() - 1;
    let j = i - n;
    evm.stack.swap(i, j);
    evm.pc += 1;
    ok(gas)
}

pub fn len(op: u8) -> usize {
    match op {
        0x60..0x80 => op as usize - 0x60 + 1, // PUSH{1..32}
        _ => 0,
    }
}

pub fn idx(op: u8) -> usize {
    match op {
        0x80..0x90 => op as usize - 0x80 + 1, // DUP{1..16}
        0x90..0xA0 => op as usize - 0x90 + 1, // SWAP{1..16}
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{call, ctx, state};
    use super::*;
    use crate::{
        Int,
        evm::{Evm, HaltReason, StepResult},
    };

    fn int(val: u64) -> Int {
        Int::from(val.to_be_bytes().as_slice())
    }

    fn is_halt(result: crate::Result<StepResult>, expected: HaltReason) -> bool {
        match (result.unwrap(), expected) {
            (StepResult::Halt(HaltReason::StackUnderflow), HaltReason::StackUnderflow) => true,
            (StepResult::Halt(HaltReason::BadOpcode(a)), HaltReason::BadOpcode(b)) => a == b,
            _ => false,
        }
    }

    // --- PUSH ---

    #[test]
    fn test_push0() {
        let mut evm = Evm::new(vec![0x5F], 1000); // PUSH0
        push(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack, vec![Int::ZERO]);
        assert_eq!(evm.pc, 1);
    }

    #[test]
    fn test_push1() {
        let mut evm = Evm::new(vec![0x60, 0x42], 1000); // PUSH1 0x42
        push(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack, vec![int(0x42)]);
        assert_eq!(evm.pc, 2);
    }

    #[test]
    fn test_push2() {
        let mut evm = Evm::new(vec![0x61, 0x01, 0x02], 1000); // PUSH2 0x0102
        push(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack, vec![int(0x0102)]);
        assert_eq!(evm.pc, 3);
    }

    #[test]
    fn test_push32() {
        let mut code = vec![0x7F]; // PUSH32
        code.extend(1u8..=32);
        let mut evm = Evm::new(code, 1000);
        push(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack.len(), 1);
        assert_eq!(evm.pc, 33);
        let expected: Vec<u8> = (1u8..=32).collect();
        assert_eq!(evm.stack[0], Int::from(expected.as_slice()));
    }

    #[test]
    fn test_push_truncated() {
        // PUSH2 with only 1 byte of immediate — should halt BadOpcode
        let mut evm = Evm::new(vec![0x61, 0xFF], 1000);
        let result = push(&mut evm, &ctx(), &call(), &mut state());
        assert!(is_halt(result, HaltReason::BadOpcode(0x61)));
    }

    // --- DUP ---

    #[test]
    fn test_dup1() {
        let a = int(1);
        let mut evm = Evm::new(vec![0x80], 1000); // DUP1
        evm.stack.push(a);
        dup(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack, vec![a, a]);
        assert_eq!(evm.pc, 1);
    }

    #[test]
    fn test_dup2() {
        let (a, b) = (int(1), int(2));
        let mut evm = Evm::new(vec![0x81], 1000); // DUP2
        evm.stack.extend([a, b]);
        dup(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack, vec![a, b, a]); // copies 2nd from top (a)
        assert_eq!(evm.pc, 1);
    }

    #[test]
    fn test_dup16() {
        let vals: Vec<Int> = (1..=16).map(int).collect();
        let mut evm = Evm::new(vec![0x8F], 1000); // DUP16
        evm.stack.extend(vals.iter().copied());
        dup(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack.len(), 17);
        assert_eq!(*evm.stack.last().unwrap(), int(1)); // copies bottom (16th from top)
    }

    #[test]
    fn test_dup_underflow() {
        let mut evm = Evm::new(vec![0x81], 1000); // DUP2, but only 1 item on stack
        evm.stack.push(int(1));
        let result = dup(&mut evm, &ctx(), &call(), &mut state());
        assert!(is_halt(result, HaltReason::StackUnderflow));
    }

    #[test]
    fn test_dup_empty() {
        let mut evm = Evm::new(vec![0x80], 1000); // DUP1 on empty stack
        let result = dup(&mut evm, &ctx(), &call(), &mut state());
        assert!(is_halt(result, HaltReason::StackUnderflow));
    }

    // --- SWAP ---

    #[test]
    fn test_swap1() {
        let (a, b) = (int(1), int(2));
        let mut evm = Evm::new(vec![0x90], 1000); // SWAP1
        evm.stack.extend([a, b]);
        swap(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack, vec![b, a]); // top swapped with 2nd
        assert_eq!(evm.pc, 1);
    }

    #[test]
    fn test_swap2() {
        let (a, b, c) = (int(1), int(2), int(3));
        let mut evm = Evm::new(vec![0x91], 1000); // SWAP2
        evm.stack.extend([a, b, c]);
        swap(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(evm.stack, vec![c, b, a]); // top swapped with 3rd
        assert_eq!(evm.pc, 1);
    }

    #[test]
    fn test_swap16() {
        let vals: Vec<Int> = (1..=17).map(int).collect(); // need 17 items for SWAP16
        let mut evm = Evm::new(vec![0x9F], 1000); // SWAP16
        evm.stack.extend(vals.iter().copied());
        swap(&mut evm, &ctx(), &call(), &mut state()).unwrap();
        assert_eq!(*evm.stack.last().unwrap(), int(1)); // bottom is now on top
        assert_eq!(evm.stack[0], int(17)); // top is now on bottom
    }

    #[test]
    fn test_swap1_underflow_empty() {
        let mut evm = Evm::new(vec![0x90], 1000); // SWAP1 on empty stack
        let result = swap(&mut evm, &ctx(), &call(), &mut state());
        assert!(is_halt(result, HaltReason::StackUnderflow));
    }

    #[test]
    fn test_swap1_underflow_one() {
        let mut evm = Evm::new(vec![0x90], 1000); // SWAP1 needs 2, has 1
        evm.stack.push(int(1));
        let result = swap(&mut evm, &ctx(), &call(), &mut state());
        assert!(is_halt(result, HaltReason::StackUnderflow));
    }

    #[test]
    fn test_swap2_underflow() {
        let mut evm = Evm::new(vec![0x91], 1000); // SWAP2 needs 3, has 2
        evm.stack.extend([int(1), int(2)]);
        let result = swap(&mut evm, &ctx(), &call(), &mut state());
        assert!(is_halt(result, HaltReason::StackUnderflow));
    }

    // --- len ---

    fn check_len(name: &str, len: usize) {
        if let Some(n) = name
            .strip_prefix("PUSH")
            .and_then(|x| x.parse::<usize>().ok())
        {
            assert_eq!(len, n, "{}", name)
        }
    }

    #[test]
    fn test_len() {
        for i in 0u8..=0xffu8 {
            let (name, _) = crate::ops::OPS[i as usize];
            let len = super::len(i);
            check_len(name, len);
        }
    }
}
