use std::collections::{HashMap, HashSet};

use yaevmi_base::{Acc, Int, dto::Head, math::lift};
use yaevmi_core::{
    state::{Account, State},
    trace::{Event, Target},
};
use yaevmi_misc::buf::Buf;

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
            nonce: Int::ZERO,
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
    pub logs: Vec<(Buf, Vec<Int>)>,
    pub events: Vec<Event>,
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
            events: vec![],
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
    fn get(&mut self, acc: &Acc, key: &Int) -> Option<(Int, Int)> {
        let entry = self.accounts.get(acc)?;
        let (cur, org) = entry.storage.get(key).map(|s| (s.current, s.original))?;
        self.emit(Event::Get(Target::Store {
            acc: *acc,
            key: *key,
            val: cur,
        }));
        Some((cur, org))
    }

    fn put(&mut self, acc: &Acc, key: &Int, val: Int) -> Option<Int> {
        let entry = self.accounts.entry(*acc).or_default();
        let slot = entry.storage.entry(*key).or_default();
        let prev = slot.current;
        slot.current = val;
        self.emit(Event::Put(
            Target::Store {
                acc: *acc,
                key: *key,
                val: prev,
            },
            val,
        ));
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

    fn tget(&mut self, key: &Int) -> Option<Int> {
        let val = self.transient.get(key).copied();
        self.emit(Event::Get(Target::Temp {
            key: *key,
            val: val.unwrap_or_default(),
        }));
        val
    }

    fn tput(&mut self, key: Int, val: Int) -> Option<Int> {
        let prev = self.transient.insert(key, val);
        self.emit(Event::Put(
            Target::Temp {
                key,
                val: prev.unwrap_or_default(),
            },
            val,
        ));
        prev
    }

    // Account mutations
    fn inc_nonce(&mut self, acc: &Acc, by: Int) -> Int {
        let (old, new) = {
            let entry = self.accounts.entry(*acc).or_default();
            let old = entry.account.nonce;
            let f = lift(|[a, b]| a + b);
            let new = f([old, by]);
            entry.account.nonce = new;
            (old, new)
        };
        self.emit(Event::Put(
            Target::Nonce {
                acc: *acc,
                val: old,
            },
            new,
        ));
        new
    }

    fn set_value(&mut self, acc: &Acc, value: Int) -> Int {
        let prev = {
            let entry = self.accounts.entry(*acc).or_default();
            let prev = entry.account.value;
            entry.account.value = value;
            prev
        };
        self.emit(Event::Put(
            Target::Value {
                acc: *acc,
                val: prev,
            },
            value,
        ));
        prev
    }

    fn set_auth(&mut self, _src: &Acc, _dst: &Acc) {
        // TODO: insert EIP-7702 delegation
    }

    fn acc_mut(&mut self, acc: &Acc) -> &mut Account {
        &mut self.accounts.entry(*acc).or_default().account
    }

    fn balance(&mut self, acc: &Acc) -> Option<Int> {
        let val = self.accounts.get(acc).map(|e| e.account.value);
        self.emit(Event::Get(Target::Value {
            acc: *acc,
            val: val.unwrap_or_default(),
        }));
        val
    }

    fn nonce(&mut self, acc: &Acc) -> Option<Int> {
        let val = self.accounts.get(acc).map(|e| e.account.nonce);
        self.emit(Event::Get(Target::Nonce {
            acc: *acc,
            val: val.unwrap_or_default(),
        }));
        val
    }

    fn code(&mut self, acc: &Acc) -> Option<(Vec<u8>, Int)> {
        let (code, hash) = self.accounts.get(acc).map(|e| e.account.code.clone())?;
        self.emit(Event::Get(Target::Code {
            acc: *acc,
            code: code.clone().into(),
        }));
        Some((code, hash))
    }

    fn acc(&mut self, acc: &Acc) -> Option<Account> {
        self.accounts.get(acc).map(|e| Account {
            value: e.account.value,
            nonce: e.account.nonce,
            code: e.account.code.clone(),
        })
    }

    fn warm_acc(&mut self, acc: &Acc) -> bool {
        let cold = self.warm_accs.insert(*acc);
        if cold {
            self.emit(Event::WarmAcc(*acc));
        }
        cold
    }

    fn warm_key(&mut self, acc: &Acc, key: &Int) -> bool {
        let cold = self.warm_keys.insert((*acc, *key));
        if cold {
            self.emit(Event::WarmKey(*acc, *key));
        }
        cold
    }

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

    fn head(&self, number: u64) -> Option<Head> {
        self.heads.get(&number).map(|&hash| Head {
            number,
            hash,
            ..Head::default()
        })
    }

    fn hash(&mut self, number: u64, hash: Int) {
        self.heads.insert(number, hash);
    }

    fn auth(&self, _acc: &Acc) -> Option<Acc> {
        None // TODO: lookup EIP-7702 delegation
    }

    fn log(&mut self, data: Buf, topics: Vec<Int>) {
        self.emit(Event::Log(topics.clone(), data.clone()));
        self.logs.push((data, topics));
    }

    fn emit(&mut self, event: Event) -> usize {
        self.events.push(event);
        self.events.len()
    }
}
