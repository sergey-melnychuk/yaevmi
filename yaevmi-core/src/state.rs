use yaevmi_misc::buf::Buf;

use crate::{Acc, Head, Int, Result, Tx, evm::Fetch, trace::Event};

pub struct Account {
    pub value: Int,
    pub nonce: Int,
    pub code: (Vec<u8>, Int),
}

pub trait State {
    fn get(&mut self, acc: &Acc, key: &Int) -> Option<(Int, Int)>;
    fn put(&mut self, acc: &Acc, key: &Int, val: Int) -> Option<Int>;
    fn init(&mut self, acc: &Acc, key: &Int, val: Int) -> Int;

    fn tget(&mut self, key: &Int) -> Option<Int>;
    fn tput(&mut self, key: Int, val: Int) -> Option<Int>;

    fn inc_nonce(&mut self, acc: &Acc, nonce: Int) -> Int;
    fn set_value(&mut self, acc: &Acc, value: Int) -> Int;
    fn set_auth(&mut self, src: &Acc, dst: &Acc);

    fn acc_mut(&mut self, acc: &Acc) -> &mut Account;

    fn balance(&mut self, acc: &Acc) -> Option<Int>;
    fn nonce(&mut self, acc: &Acc) -> Option<Int>;
    fn code(&mut self, acc: &Acc) -> Option<(Vec<u8>, Int)>;
    fn acc(&mut self, acc: &Acc) -> Option<Account>;

    fn warm_acc(&mut self, acc: &Acc) -> bool;
    fn warm_key(&mut self, acc: &Acc, key: &Int) -> bool;

    fn create(&mut self, acc: Acc, info: Account);
    fn destroy(&mut self, acc: &Acc);

    fn hash(&mut self, number: u64, hash: Int);
    fn log(&mut self, data: Buf, topics: Vec<Int>);

    fn head(&self, number: u64) -> Option<Head>;
    fn auth(&self, acc: &Acc) -> Option<Acc>;
    fn created(&self) -> &[Acc];
    fn destroyed(&self) -> &[Acc];

    fn emit(&mut self, event: Event) -> usize;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait Chain {
    async fn get(&self, acc: &Acc, key: &Int) -> eyre::Result<Int>;
    async fn acc(&self, acc: &Acc) -> eyre::Result<Account>;
    async fn code(&self, acc: &Acc) -> eyre::Result<(Vec<u8>, Int)>;
    async fn nonce(&self, acc: &Acc) -> eyre::Result<u64>;
    async fn balance(&self, acc: &Acc) -> eyre::Result<Int>;
    async fn head(&self, number: u64) -> eyre::Result<Head>;
    async fn block(&self, number: u64) -> eyre::Result<(Head, Vec<Tx>)>;
}

pub async fn fetch(f: Fetch, state: &mut impl State, chain: &impl Chain) -> Result<()> {
    match f {
        Fetch::Account(acc) => {
            let account = chain.acc(&acc).await?;
            *state.acc_mut(&acc) = account;
        }
        Fetch::Balance(acc) => {
            let account = chain.acc(&acc).await?;
            *state.acc_mut(&acc) = account;
        }
        Fetch::Nonce(acc) => {
            let account = chain.acc(&acc).await?;
            *state.acc_mut(&acc) = account;
        }
        Fetch::Code(acc) => {
            let account = chain.acc(&acc).await?;
            *state.acc_mut(&acc) = account;
        }
        Fetch::BlockHash(number) => {
            let head = chain.head(number).await?;
            state.hash(number, head.hash);
        }
        Fetch::StateCell(acc, key) => {
            let val = chain.get(&acc, &key).await?;
            state.init(&acc, &key, val);
        }
    }
    Ok(())
}
