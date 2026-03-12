use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use yaevmi_base::{
    Acc, Int,
    dto::{Head, Tx},
};
use yaevmi_misc::buf::Buf;

pub mod aux;
pub mod cache;
pub mod chain;
pub mod evm;
pub mod exe;
pub mod ops;
pub mod pre;
pub mod rpc;
pub mod state;
pub mod trace;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Code missing: {0:?}")]
    MissingCode(Acc),
    #[error("Call result missing")]
    CallResultMissing,
    #[error("Generic error: {0}")]
    Generic(#[from] eyre::Report),
    #[error("Internal error: {0}")]
    Internal(Cow<'static, str>),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Call {
    pub by: Acc,
    pub to: Acc,
    pub gas: u64,
    pub eth: Int,
    pub data: Buf,
    pub auth: Vec<Acc>,
    pub nonce: Option<u64>,
}
