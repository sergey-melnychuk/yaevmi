#[cfg(test)]
pub mod eth;

#[cfg(test)]
pub mod revm;

#[cfg(test)]
pub mod sol;

// git clone --depth 1 https://github.com/ethereum/tests yaevmi-test/tests 2>&1
// cd yaevmi-test/tests && tar -xzf fixtures_general_state_tests.tgz && rm -rf .git
// cargo test -p yaevmi-test
