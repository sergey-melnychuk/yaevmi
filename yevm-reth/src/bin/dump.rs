//! Dump the full state of one contract (balance, nonce, code, every storage
//! slot) straight from a local reth datadir, no RPC involved.
//!
//! Adapted from reth's own `examples/full-contract-state`
//! (<https://github.com/paradigmxyz/reth/tree/v2.5.1/examples/full-contract-state>).
//!
//! Usage:
//!   RETH_DATADIR=~/Temp/reth-node/datadir CONTRACT_ADDRESS=0x... cargo run -p yevm-reth --bin dump

use std::str::FromStr;

use alloy_primitives::{Address, U256, keccak256, map::B256Map};
use reth_ethereum::{
    chainspec::ChainSpecBuilder,
    node::EthereumNode,
    primitives::{Account, Bytecode},
    provider::{
        ProviderResult,
        db::{
            cursor::{DbCursorRO, DbDupCursorRO},
            tables,
            transaction::DbTx,
        },
        providers::ReadOnlyConfig,
    },
    storage::{DBProvider, StateProvider, StorageSettingsCache},
};

/// Balance/nonce/code + every storage slot for one contract.
#[derive(Debug, Clone)]
pub struct ContractState {
    pub address: Address,
    pub account: Account,
    pub bytecode: Option<Bytecode>,
    /// `keccak256(slot) -> value` when the node runs in hashed-state mode
    /// (see `hashed` below), `slot -> value` otherwise.
    pub storage: B256Map<U256>,
    /// True if `storage` is keyed by `keccak256(slot)`, not the raw slot.
    pub hashed: bool,
}

/// Point lookups for balance/nonce/code, then a dup-cursor range scan for
/// every slot belonging to `contract_address` -- the same tables
/// `eth_getStorageAt` reads one key at a time, read here as one sorted run
/// instead of N point lookups.
///
/// A live node keeps *either* `PlainStorageState` (keyed by the raw 32-byte
/// slot) *or* `HashedStorages` (keyed by `keccak256(slot)`) up to date, per
/// `cached_storage_settings().use_hashed_state()` -- reth's own
/// `StateProvider::storage()` branches on exactly this flag
/// (`providers/state/latest.rs`). Get it wrong and the scan silently
/// returns zero rows instead of an error, because the table itself is
/// simply not maintained in that mode -- there's nothing to fail on.
/// Hashed mode has no plain-key mirror to read the raw slot number back
/// from (that's the space it saves), so the dump falls back to listing
/// `keccak256(slot) -> value` pairs instead of `slot -> value`.
pub fn extract_contract_state<P: DBProvider + StorageSettingsCache>(
    provider: &P,
    state_provider: &dyn StateProvider,
    contract_address: Address,
) -> ProviderResult<Option<ContractState>> {
    let account = state_provider.basic_account(&contract_address)?;
    let Some(account) = account else {
        return Ok(None);
    };

    let bytecode = state_provider.account_code(&contract_address)?;

    let hashed = provider.cached_storage_settings().use_hashed_state();
    let mut storage = B256Map::default();

    if hashed {
        let hashed_address = keccak256(contract_address);
        let mut cursor = provider
            .tx_ref()
            .cursor_dup_read::<tables::HashedStorages>()?;
        if let Some((_, first_entry)) = cursor.seek_exact(hashed_address)? {
            storage.insert(first_entry.key, first_entry.value);
            while let Some((_, entry)) = cursor.next_dup()? {
                storage.insert(entry.key, entry.value);
            }
        }
    } else {
        let mut cursor = provider
            .tx_ref()
            .cursor_dup_read::<tables::PlainStorageState>()?;
        if let Some((_, first_entry)) = cursor.seek_exact(contract_address)? {
            storage.insert(first_entry.key, first_entry.value);
            while let Some((_, entry)) = cursor.next_dup()? {
                storage.insert(entry.key, entry.value);
            }
        }
    }

    Ok(Some(ContractState {
        address: contract_address,
        account,
        bytecode,
        storage,
        hashed,
    }))
}

fn main() -> eyre::Result<()> {
    let address = std::env::var("CONTRACT_ADDRESS")?;
    let contract_address = Address::from_str(&address)?;

    let datadir = std::env::var("RETH_DATADIR")?;
    let spec = ChainSpecBuilder::mainnet().build();
    let runtime = reth_ethereum::tasks::Runtime::test();
    let factory = EthereumNode::provider_factory_builder().open_read_only(
        spec.into(),
        ReadOnlyConfig::from_datadir(datadir),
        runtime,
    )?;

    let provider = factory.provider()?;
    let state_provider = factory.latest()?;
    let contract_state =
        extract_contract_state(&provider, state_provider.as_ref(), contract_address)?;

    let Some(state) = contract_state else {
        println!("No account found at {contract_address}");
        return Ok(());
    };

    println!("Contract: {}", state.address);
    println!("Balance: {}", state.account.balance);
    println!("Nonce: {}", state.account.nonce);
    println!("Code hash: {:?}", state.account.bytecode_hash);
    println!(
        "Code size: {} bytes",
        state.bytecode.map(|b| b.len()).unwrap_or(0)
    );

    if state.storage.is_empty() {
        return Ok(());
    }

    if state.hashed {
        println!(
            "Storage slots: {} (hashed-state node -- keys below are keccak256(slot), \
                     not the raw slot number)",
            state.storage.len()
        );
        for (hashed_key, value) in &state.storage {
            println!("\tkeccak256(slot)={hashed_key}: {value}");
        }
    } else {
        println!("Storage slots: {}", state.storage.len());
        for (key, value) in &state.storage {
            println!("\t{key}: {value}");
        }
    }

    Ok(())
}
