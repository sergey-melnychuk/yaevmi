use yaevmi_base::{Acc, Int};
use yaevmi_misc::buf::Buf;

use crate::{
    Call,
    evm::{CallMode, HaltReason},
};

pub enum Target {
    Nonce { acc: Acc, val: Int },
    Value { acc: Acc, val: Int },
    Store { acc: Acc, key: Int, val: Int },
    Temp { key: Int, val: Int },
    Code { acc: Acc, code: Buf },
    Auth { acc: Acc },
}

pub enum Event {
    WarmKey(Acc, Int),
    WarmAcc(Acc),
    Move(Acc, Acc, Int),
    Get(Target),
    Put(Target, Int),
    Hash(Buf, Int),
    Log(Vec<Int>, Buf),
    Call(Call, CallMode),
    Return(Buf),
    Revert(Buf),
    Create(Acc),
    Delete(Acc),
    Fee(Acc, Int, Int),
    Halt(HaltReason),
    Blob(u64, Int), // EIP-4844 BLOB carrying txs

    Step(usize, (u8, Option<Int>), u64),
    Full(usize, u8, (u64, u64), Vec<Int>, Buf),
}
