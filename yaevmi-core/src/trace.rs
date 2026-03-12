use serde::{Deserialize, Serialize};
use yaevmi_base::{Acc, Int};
use yaevmi_misc::buf::Buf;

use crate::{
    Call,
    evm::{CallMode, HaltReason},
};

#[derive(Debug, Deserialize, Serialize)]
pub enum Target {
    Nonce { acc: Acc, val: Int },
    Value { acc: Acc, val: Int },
    Store { acc: Acc, key: Int, val: Int },
    Temp { key: Int, val: Int },
    Code { acc: Acc, hash: Int },
    Auth { acc: Acc },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Event {
    WarmKey(Acc, Int),
    WarmAcc(Acc),
    Move(Acc, Acc, Int),
    Get(Target),
    Put(Target, Int),
    Hash(Buf, Int),
    Code(Buf, Int),
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Trace {
    pub id: usize,
    pub event: Event,
    pub depth: usize,
    pub reverted: bool,
}
