# yaevmi

**(Yield-Aware | Yet Another) EVM Implementation** — in Rust.

## Goals

- **Async-first** — execution yields on every external state access (balance, storage, code), letting the caller fetch lazily from any source (RPC, local DB, mock)
- **WebAssembly-friendly** — runs in-browser or in edge environments without modification
- **Correctness & observability** over raw performance — full tracing, clean error types, inspectable state at every step
- **Zero infrastructure** — simulate any transaction against a forked or mock state without running a node

Intended for: educational tooling, transaction debugging, independent simulation, and testing infrastructure. Forking real chain state requires an RPC node (e.g. via `eth_getStorageAt`, `eth_getBalance`, etc.).

## Crates

| Crate | Description |
|---|---|
| `yaevmi-base` | Primitive types: `Acc` (address), `Int` (uint256), `Head`, `Tx` |
| `yaevmi-core` | EVM engine: opcode dispatch, stack/memory, `State`/`Chain` traits |
| `yaevmi-misc` | Utilities and helpers |
| `yaevmi-wasm` | WebAssembly bindings |
| `yaevmi-test` | Test harness and fixtures |
| `yaevmi-full` | Full integration: ties all crates together |
