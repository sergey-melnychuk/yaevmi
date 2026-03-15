mod ecrecover;

/// Run a precompile. Returns `(success, output, gas_used)`.
/// `success=false` means out-of-gas; output is empty and gas_used equals gas_limit.
pub fn run(id: u64, input: &[u8], gas_limit: i64) -> (bool, Vec<u8>, i64) {
    match id {
        1 => ecrecover::ecrecover(input, gas_limit),
        // 2-9: unimplemented — succeed with empty output and zero gas cost
        _ => (true, vec![], 0),
    }
}
