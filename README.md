# yaevmi

**(Yield-Aware | Yet Another) EVM Implementation** — in Rust.

## Goals

- **Async-first** — execution yields on every external state access (balance, storage, code), letting the caller fetch lazily from any source (RPC, local DB, mock)
- **WebAssembly-friendly** — runs in-browser or in edge environments without modification
- **Correctness & observability** over raw performance — full tracing, clean error types, inspectable state at every step
- **Zero infrastructure** — simulate any transaction against a forked or mock state without running a node

Intended for: educational tooling, transaction debugging, independent simulation, and testing infrastructure. Forking real chain state requires an RPC node (e.g. for `eth_getStorageAt`, `eth_getBalance`, etc.).

## Crates

| Crate | Description |
|---|---|
| `yaevmi-base` | Primitive types: `Acc` (address), `Int` (uint256), `Head`, `Tx` |
| `yaevmi-core` | EVM engine: opcode dispatch, stack/memory, `State`/`Chain` traits |
| `yaevmi-misc` | Utilities and helpers |
| `yaevmi-wasm` | WebAssembly bindings |
| `yaevmi-test` | Test harness and fixtures |
| `yaevmi-full` | Full integration: ties all crates together |

## Tests

Running against `GeneralStateTests`:

```bash
cargo test -p yaevmi-test

## Make sure you have test fixtures cloned and extracted locally:
git clone --depth 1 https://github.com/ethereum/tests yaevmi-test/tests
cd yaevmi-test/tests && tar -xzf fixtures_general_state_tests.tgz
```

## WebAssembly

Web is a first-class build & usage target:

```bash
cd yaevmi-wasm
wasm-pack build --target web
python3 -m http.server
## open http://localhost:8000/
```

## Links

1. [YellowPaper](https://ethereum.github.io/yellowpaper/paper.pdf)

2. [EVM.codes](https://www.evm.codes/)
