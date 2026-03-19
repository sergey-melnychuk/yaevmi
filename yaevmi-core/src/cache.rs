use std::collections::{HashMap, HashSet};

use crate::{
    call::Head,
    state::{Account, State},
    trace::{Event, Target, Trace},
};
use yaevmi_base::{Acc, Int, math::lift};
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
            code: (vec![].into(), Int::ZERO),
        })
    }
}

enum Revert {
    WarmAcc(Acc),
    WarmKey(Acc, Int),
    Store(Acc, Int, Int),
    Nonce(Acc, Int),
    Value(Acc, Int),
    Temp(Acc, Int, Int),
    Code(Acc, Int),
    Create(Acc),
    Delete(Acc),
}

#[derive(Default)]
pub struct Cache {
    accounts: HashMap<Acc, AccountEntry>,
    transient: HashMap<(Acc, Int), Int>,
    warm_accs: HashSet<Acc>,
    warm_keys: HashSet<(Acc, Int)>,
    created: HashSet<Acc>,
    destroyed: HashSet<Acc>,
    hash: HashMap<u64, Int>,
    depth: usize,
    pub logs: Vec<(Buf, Vec<Int>)>,
    pub events: Vec<Trace>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            transient: HashMap::new(),
            warm_accs: HashSet::new(),
            warm_keys: HashSet::new(),
            created: HashSet::new(),
            destroyed: HashSet::new(),
            hash: HashMap::new(),
            depth: 0,
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

impl State for Cache {
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

    fn tget(&mut self, acc: &Acc, key: &Int) -> Option<Int> {
        let val = self.transient.get(&(*acc, *key)).copied();
        self.emit(Event::Get(Target::Temp {
            acc: *acc,
            key: *key,
            val: val.unwrap_or_default(),
        }));
        val
    }

    fn tput(&mut self, acc: Acc, key: Int, val: Int) -> Option<Int> {
        let prev = self.transient.insert((acc, key), val);
        self.emit(Event::Put(
            Target::Temp {
                acc,
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

    // fn set_code(&mut self, acc: &Acc, code: Buf, hash: Int) -> Int {
    //     let prev = {
    //         let entry = self.accounts.entry(*acc).or_default();
    //         let prev = entry.account.value;
    //         entry.account.value = value;
    //         prev
    //     };
    //     self.emit(Event::Put(
    //         Target::Value {
    //             acc: *acc,
    //             val: prev,
    //         },
    //         value,
    //     ));
    //     prev
    // }

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

    fn code(&mut self, acc: &Acc) -> Option<(Buf, Int)> {
        let (code, hash) = self.accounts.get(acc).map(|e| e.account.code.clone())?;
        self.emit(Event::Get(Target::Code { acc: *acc, hash }));
        Some((code, hash))
    }

    fn acc(&mut self, acc: &Acc) -> Option<Account> {
        self.accounts.get(acc).map(|e| Account {
            value: e.account.value,
            nonce: e.account.nonce,
            code: e.account.code.clone(),
        })
    }

    fn is_cold_acc(&self, acc: &Acc) -> bool {
        !self.warm_accs.contains(acc)
    }

    fn is_cold_key(&self, acc: &Acc, key: &Int) -> bool {
        !self.warm_keys.contains(&(*acc, *key))
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
        self.emit(Event::Put(
            Target::Nonce {
                acc,
                val: Int::ZERO,
            },
            info.nonce,
        ));
        self.emit(Event::Put(
            Target::Value {
                acc,
                val: Int::ZERO,
            },
            info.value,
        ));
        self.emit(Event::Create(acc));
        self.accounts.insert(acc, AccountEntry::new(info));
        self.created.insert(acc);
    }

    fn destroy(&mut self, acc: &Acc) {
        self.destroyed.insert(*acc);
        self.emit(Event::Delete(*acc));
    }

    fn created(&self) -> Vec<Acc> {
        self.created.iter().cloned().collect()
    }

    fn destroyed(&self) -> Vec<Acc> {
        self.destroyed.iter().cloned().collect()
    }

    fn head(&self, number: u64) -> Option<Head> {
        self.hash.get(&number).map(|&hash| Head {
            number,
            hash,
            ..Head::default()
        })
    }

    fn hash(&mut self, number: u64, hash: Int) {
        self.hash.insert(number, hash);
    }

    fn auth(&self, _acc: &Acc) -> Option<Acc> {
        None // TODO: lookup EIP-7702 delegation
    }

    fn log(&mut self, data: Buf, topics: Vec<Int>) {
        self.emit(Event::Log(topics.clone(), data.clone()));
        self.logs.push((data, topics));
    }

    fn set_depth(&mut self, depth: usize) {
        self.depth = depth;
    }

    fn emit(&mut self, mut event: Event) -> usize {
        let id = self.events.len();
        if let Event::Step(step) = &mut event {
            step.debug.push(format!("depth={}", self.depth));
        }
        self.events.push(Trace {
            seq: id,
            event,
            depth: self.depth,
            reverted: false,
        });
        id
    }

    fn checkpoint(&mut self) -> usize {
        self.events.len()
    }

    fn revert_to(&mut self, cp: usize) {
        if cp >= self.events.len() {
            return;
        }

        // Count logs added since checkpoint (to truncate self.logs)
        let logs_to_remove = self.events[cp..]
            .iter()
            .filter(|t| !t.reverted)
            .filter(|t| matches!(t.event, Event::Log(..)))
            .count();
        self.logs
            .truncate(self.logs.len().saturating_sub(logs_to_remove));

        // Collect undo operations (immutable borrow of events ends here)
        let undos: Vec<Revert> = self.events[cp..]
            .iter()
            .filter(|t| !t.reverted)
            .filter_map(|trace| match &trace.event {
                Event::Put(Target::Store { acc, key, val }, _) => {
                    Some(Revert::Store(*acc, *key, *val))
                }
                Event::Put(Target::Nonce { acc, val }, _) => Some(Revert::Nonce(*acc, *val)),
                Event::Put(Target::Value { acc, val }, _) => Some(Revert::Value(*acc, *val)),
                Event::Put(Target::Temp { acc, key, val }, _) => {
                    Some(Revert::Temp(*acc, *key, *val))
                }
                Event::Put(Target::Code { acc, hash }, _) => Some(Revert::Code(*acc, *hash)),
                Event::WarmAcc(acc) => Some(Revert::WarmAcc(*acc)),
                Event::WarmKey(acc, key) => Some(Revert::WarmKey(*acc, *key)),
                Event::Create(acc) => Some(Revert::Create(*acc)),
                Event::Delete(acc) => Some(Revert::Delete(*acc)),
                _ => None,
            })
            .collect();

        // Mark all reverted traces
        for t in &mut self.events[cp..] {
            t.reverted = true;
        }

        // Apply undos in reverse order
        for undo in undos.into_iter().rev() {
            match undo {
                Revert::Store(acc, key, val) => {
                    if let Some(entry) = self.accounts.get_mut(&acc)
                        && let Some(slot) = entry.storage.get_mut(&key)
                    {
                        slot.current = val;
                    }
                }
                Revert::Nonce(acc, val) => {
                    if let Some(entry) = self.accounts.get_mut(&acc) {
                        entry.account.nonce = val;
                    }
                }
                Revert::Value(acc, val) => {
                    if let Some(entry) = self.accounts.get_mut(&acc) {
                        entry.account.value = val;
                    }
                }
                Revert::Temp(acc, key, val) => {
                    if val.is_zero() {
                        self.transient.remove(&(acc, key));
                    } else {
                        self.transient.insert((acc, key), val);
                    }
                }
                Revert::Code(acc, _prev_hash) => {
                    if let Some(entry) = self.accounts.get_mut(&acc) {
                        entry.account.code = (Buf::default(), Int::ZERO);
                    }
                }
                Revert::Create(acc) => {
                    self.created.remove(&acc);
                }
                Revert::Delete(acc) => {
                    self.destroyed.remove(&acc);
                }
                Revert::WarmAcc(acc) => {
                    self.warm_accs.remove(&acc);
                }
                Revert::WarmKey(acc, key) => {
                    self.warm_keys.remove(&(acc, key));
                }
            }
        }
    }

    fn apply(&mut self) {
        let destroyed = std::mem::take(&mut self.destroyed);
        for acc in destroyed {
            if let Some(entry) = self.accounts.get_mut(&acc) {
                entry.account.value = Int::ZERO;
                entry.account.nonce = Int::ZERO;
                entry.account.code = (Buf::default(), Int::ZERO);
                entry.storage.clear();
            }
            self.created.remove(&acc);
        }
    }
}

pub type Env = Vec<(Acc, Account, Vec<(Int, Int)>)>;

impl Cache {
    pub fn snapshot(&self) -> Env {
        let mut ret = Vec::with_capacity(self.accounts.len());
        for (acc, entry) in &self.accounts {
            let mut kv = Vec::with_capacity(entry.storage.len());
            for (key, slot) in &entry.storage {
                kv.push((*key, slot.current));
            }
            kv.sort_by_key(|(k, _)| *k);
            ret.push((*acc, entry.account.clone(), kv));
        }
        ret.sort_by_key(|(acc, _, _)| *acc);
        ret
    }
}
