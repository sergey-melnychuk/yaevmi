use serde::{Deserialize, Serialize};
use yaevmi_base::{Acc, Int};
use yaevmi_misc::buf::Buf;

use crate::{
    Call,
    evm::{CallMode, HaltReason},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Target {
    Nonce { acc: Acc, val: Int },
    Value { acc: Acc, val: Int },
    Store { acc: Acc, key: Int, val: Int },
    Temp { key: Int, val: Int },
    Code { acc: Acc, hash: Int },
    Auth { acc: Acc },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

    Step(Step),
    Full(Step, Vec<Int>, Buf),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Step {
    pub pc: usize,
    pub op: u8,
    pub name: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Buf>,
    pub gas: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Trace {
    pub seq: usize,
    pub event: Event,
    pub depth: usize,
    pub reverted: bool,
}
