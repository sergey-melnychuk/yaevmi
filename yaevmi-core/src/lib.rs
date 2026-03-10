use std::borrow::Cow;

use thiserror::Error;

pub use yaevmi_base::{
    Acc, Int,
    dto::{Head, Tx},
};

pub mod evm;
pub mod exe;
pub mod ops;
pub mod pre;
pub mod rpc;
pub mod state;
pub mod trace;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Internal error: {0}")]
    Internal(Cow<'static, str>),
    #[error("Generic error: {0}")]
    Generic(#[from] eyre::Report),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Call {
    pub by: Acc,
    pub to: Acc,
    pub gas: u64,
    pub eth: Int,
    pub data: Vec<u8>,
    pub auth: Vec<Acc>,
    pub nonce: Option<u64>,
}
