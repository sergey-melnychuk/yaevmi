#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Acc([u8; 20]);
// TODO: serde, into/from hex, as_ref
// TODO: into(Int)

impl Acc {
    pub const ZERO: Self = Acc([0; 20]);

    pub fn to_int(&self) -> Int {
        let mut int = Int::ZERO;
        int.0[12..].copy_from_slice(&self.0);
        int
    }

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|byte| byte == &0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Int([u8; 32]);
// TODO: serde, into/from hex, as_ref
// TODO: arithmetics: (un)signed/modular/bitwise
// TODO: arithmetics: overflow/wrapping
// TODO: from(Acc), as_u64/i64, as_usize/isize

impl Int {
    pub const ZERO: Self = Int([0; 32]);
    pub const ONE: Self = Int([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1,
    ]);

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|byte| byte == &0)
    }
}

impl From<&[u8]> for Int {
    fn from(value: &[u8]) -> Self {
        let mut buffer = [0u8; 32];
        let skip = 32 - value.len();
        buffer[skip..].copy_from_slice(value);
        Self(buffer)
    }
}

// TODO: RPC DTOs: block, header, tx

// TODO: tracing DTOs (events, touches, etc)

pub struct Head {
    pub number: u64,
    pub hash: Int,
    // TODO
}

pub struct Tx {
    // TODO
}
