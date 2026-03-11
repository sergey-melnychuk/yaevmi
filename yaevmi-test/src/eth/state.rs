use std::collections::{HashMap, HashSet};

use yaevmi_base::{Acc, Int, dto::Head};
use yaevmi_core::state::{Account, State};

#[derive(Default)]
struct Slot {
    original: Int,
    current: Int,
}

struct AccountEntry {
    account: Account,
    storage: HashMap<Int, Slot>,
}

impl AccountEntry {
    fn new(account: Account) -> Self {
        Self {
            account,
            storage: HashMap::new(),
        }
    }
}

impl Default for AccountEntry {
    fn default() -> Self {
        Self::new(Account {
            value: Int::ZERO,
            nonce: 0,
            code: (vec![], Int::ZERO),
        })
    }
}

#[derive(Default)]
pub struct InMemoryState {
    accounts: HashMap<Acc, AccountEntry>,
    transient: HashMap<Int, Int>,
    warm_accs: HashSet<Acc>,
    warm_keys: HashSet<(Acc, Int)>,
    created: Vec<Acc>,
    destroyed: Vec<Acc>,
    heads: HashMap<u64, Int>,
    pub logs: Vec<(Vec<u8>, Vec<Int>)>,
}

impl InMemoryState {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            transient: HashMap::new(),
            warm_accs: HashSet::new(),
            warm_keys: HashSet::new(),
            created: vec![],
            destroyed: vec![],
            heads: HashMap::new(),
            logs: vec![],
        }
    }

    pub fn insert_account(&mut self, addr: Acc, account: Account) {
        self.accounts.insert(addr, AccountEntry::new(account));
    }

    pub fn insert_storage(&mut self, addr: Acc, key: Int, val: Int) {
        let entry = self.accounts.entry(addr).or_default();
        entry.storage.insert(
            key,
            Slot {
                original: val,
                current: val,
            },
        );
    }

    pub fn account(&self, addr: &Acc) -> Option<&Account> {
        self.accounts.get(addr).map(|e| &e.account)
    }

    pub fn storage(&self, addr: &Acc, key: &Int) -> Option<Int> {
        self.accounts.get(addr)?.storage.get(key).map(|s| s.current)
    }
}

impl State for InMemoryState {
    // Storage: returns (current, original)
    fn get(&self, acc: &Acc, key: &Int) -> Option<(Int, Int)> {
        let entry = self.accounts.get(acc)?;
        entry.storage.get(key).map(|s| (s.current, s.original))
    }

    fn put(&mut self, acc: &Acc, key: &Int, val: Int) -> Option<Int> {
        let entry = self.accounts.entry(*acc).or_default();
        let slot = entry.storage.entry(*key).or_default();
        let prev = slot.current;
        slot.current = val;
        Some(prev)
    }

    fn init(&mut self, acc: &Acc, key: &Int, val: Int) -> Int {
        let entry = self.accounts.entry(*acc).or_default();
        entry.storage.insert(
            *key,
            Slot {
                original: val,
                current: val,
            },
        );
        val
    }

    // Transient storage (EIP-1153)
    fn tget(&self, key: &Int) -> Option<Int> {
        self.transient.get(key).copied()
    }

    fn tput(&mut self, key: Int, val: Int) -> Option<Int> {
        self.transient.insert(key, val)
    }

    // Account mutations
    fn inc_nonce(&mut self, acc: &Acc, by: u64) -> u64 {
        let entry = self.accounts.entry(*acc).or_default();
        eprintln!("DEBUG: inc nonce={}", entry.account.nonce); // TODO: FIXME: !!!
        entry.account.nonce += by;
        entry.account.nonce
    }

    fn set_value(&mut self, acc: &Acc, value: Int) -> Int {
        let entry = self.accounts.entry(*acc).or_default();
        let prev = entry.account.value;
        entry.account.value = value;
        prev
    }

    fn acc_mut(&mut self, acc: &Acc) -> &mut Account {
        &mut self.accounts.entry(*acc).or_default().account
    }

    // Account queries
    fn balance(&self, acc: &Acc) -> Option<Int> {
        self.accounts.get(acc).map(|e| e.account.value)
    }

    fn nonce(&self, acc: &Acc) -> Option<u64> {
        self.accounts.get(acc).map(|e| e.account.nonce)
    }

    fn code(&self, acc: &Acc) -> Option<(Vec<u8>, Int)> {
        self.accounts.get(acc).map(|e| e.account.code.clone())
    }

    fn acc(&self, acc: &Acc) -> Option<Account> {
        self.accounts.get(acc).map(|e| Account {
            value: e.account.value,
            nonce: e.account.nonce,
            code: e.account.code.clone(),
        })
    }

    // Access list / warmth
    fn is_warm_acc(&self, acc: &Acc) -> bool {
        self.warm_accs.contains(acc)
    }

    fn is_warm_key(&self, acc: &Acc, key: &Int) -> bool {
        self.warm_keys.contains(&(*acc, *key))
    }

    fn warm_acc(&mut self, acc: &Acc) -> bool {
        !self.warm_accs.insert(*acc)
    }

    fn warm_key(&mut self, acc: &Acc, key: &Int) -> bool {
        !self.warm_keys.insert((*acc, *key))
    }

    // Account lifecycle
    fn create(&mut self, acc: Acc, info: Account) {
        self.accounts.insert(acc, AccountEntry::new(info));
        self.created.push(acc);
    }

    fn destroy(&mut self, acc: &Acc) {
        self.accounts.remove(acc);
        self.destroyed.push(*acc);
    }

    fn created(&self) -> &[Acc] {
        &self.created
    }

    fn destroyed(&self) -> &[Acc] {
        &self.destroyed
    }

    // Block headers
    fn head(&self, number: u64) -> Option<Head> {
        self.heads.get(&number).map(|&hash| Head {
            number,
            hash,
            ..Head::default()
        })
    }

    fn set_hash(&mut self, number: u64, hash: Int) {
        self.heads.insert(number, hash);
    }

    // EIP-7702 delegation
    fn get_delegation(&mut self, _acc: &Acc) -> Option<Acc> {
        None
    }

    // Logs
    fn log(&mut self, data: Vec<u8>, topics: Vec<Int>) {
        self.logs.push((data, topics));
    }
}
