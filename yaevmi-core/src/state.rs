use yaevmi_misc::buf::Buf;

use crate::{Acc, Head, Int, trace::Event};

pub struct Account {
    pub value: Int,
    pub nonce: Int,
    pub code: (Buf, Int),
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
    fn code(&mut self, acc: &Acc) -> Option<(Buf, Int)>;
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

    /// Take a state checkpoint; returns an opaque ID that can be passed to `revert_to`.
    fn checkpoint(&mut self) -> usize {
        0
    }
    /// Revert all state mutations since the given checkpoint.
    fn revert_to(&mut self, _checkpoint: usize) {}
}
