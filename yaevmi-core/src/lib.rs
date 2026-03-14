use std::borrow::Cow;

use thiserror::Error;

pub use yaevmi_base::{Acc, Int};

pub use crate::call::{Call, Head, Tx};
pub use crate::evm::Fetch;

pub mod aux;
pub mod cache;
pub mod call;
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
    #[error("Data missing: {0:?}")]
    MissingData(Fetch),
    #[error("Call result missing")]
    CallResultMissing,
    #[error("Inconsistent state")]
    InconsistentState,
    #[error("Generic error: {0}")]
    Generic(#[from] eyre::Report),
    #[error("Internal error: {0}")]
    Internal(Cow<'static, str>),
}

pub type Result<T> = std::result::Result<T, Error>;
