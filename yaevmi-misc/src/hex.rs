#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hex<const N: usize>([u8; N]);

impl<const N: usize> Hex<N> {
    pub const N: usize = N;
    pub const ZERO: Self = Self::zero();
    pub const ONE: Self = Self::one();

    pub const fn new(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    pub const fn zero() -> Self {
        Self([0; N])
    }

    pub const fn one() -> Self {
        let mut buf = [0; N];
        buf[N - 1] = 1;
        Self(buf)
    }

    pub fn is_zero(&self) -> bool {
        self == &Self::ZERO
    }

    pub fn as_usize(&self) -> usize {
        const K: usize = size_of::<usize>();
        let mut b = [0u8; K];
        b.copy_from_slice(&self.0[N - K..]);
        usize::from_be_bytes(b)
    }

    pub fn as_u128(&self) -> u128 {
        const K: usize = size_of::<u128>();
        let mut b = [0u8; K];
        b.copy_from_slice(&self.0[N - K..]);
        u128::from_be_bytes(b)
    }

    pub fn as_u64(&self) -> u64 {
        const K: usize = size_of::<u64>();
        let mut b = [0u8; K];
        b.copy_from_slice(&self.0[N - K..]);
        u64::from_be_bytes(b)
    }

    pub fn as_u32(&self) -> u32 {
        const K: usize = size_of::<u32>();
        let mut b = [0u8; K];
        b.copy_from_slice(&self.0[N - K..]);
        u32::from_be_bytes(b)
    }

    pub fn as_u16(&self) -> u16 {
        const K: usize = size_of::<u16>();
        let mut b = [0u8; K];
        b.copy_from_slice(&self.0[N - K..]);
        u16::from_be_bytes(b)
    }

    pub fn as_u8(&self) -> u8 {
        self.0[N - 1]
    }
}

impl<const N: usize> AsRef<[u8]> for Hex<N> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const N: usize> From<&[u8]> for Hex<N> {
    fn from(value: &[u8]) -> Self {
        assert!(value.len() <= N, "data loss detected");
        let mut buffer = [0u8; N];
        let take = value.len().min(N);
        let skip = N - take;
        buffer[skip..].copy_from_slice(&value[..take]);
        Self(buffer)
    }
}

impl<const N: usize> From<usize> for Hex<N> {
    fn from(value: usize) -> Self {
        let buffer = value.to_be_bytes();
        Self::from(&buffer[..])
    }
}

impl<const N: usize> From<u128> for Hex<N> {
    fn from(value: u128) -> Self {
        let buffer = value.to_be_bytes();
        Self::from(&buffer[..])
    }
}

impl<const N: usize> From<u64> for Hex<N> {
    fn from(value: u64) -> Self {
        let buffer = value.to_be_bytes();
        Self::from(&buffer[..])
    }
}

impl<const N: usize> From<u32> for Hex<N> {
    fn from(value: u32) -> Self {
        let buffer = value.to_be_bytes();
        Self::from(&buffer[..])
    }
}

impl<const N: usize> From<u16> for Hex<N> {
    fn from(value: u16) -> Self {
        let buffer = value.to_be_bytes();
        Self::from(&buffer[..])
    }
}

impl<const N: usize> From<u8> for Hex<N> {
    fn from(value: u8) -> Self {
        let buffer = [value];
        Self::from(&buffer[..])
    }
}

impl<const N: usize> Hex<N> {
    pub fn to<const M: usize>(self) -> Hex<M> {
        let mut buffer = [0; M];
        if M > N {
            buffer[(M - N)..].copy_from_slice(&self.0);
        } else {
            buffer[..].copy_from_slice(&self.0[(N - M)..]);
        }
        Hex(buffer)
    }
}

impl<const N: usize> Default for Hex<N> {
    fn default() -> Self {
        let buffer = [0; N];
        Self(buffer)
    }
}

pub const fn parse<const N: usize>(s: &str) -> [u8; N] {
    if s.len() > N * 2 {
        panic!("hex literal too long");
    }
    let offset = N * 2 - s.len();
    let mut ret = [0u8; N];
    let mut j = 0;
    while j < s.len() {
        let c = s.as_bytes()[j];
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => panic!("hex literal invalid"),
        };
        let i = offset + j;
        let b = i / 2;
        if i.is_multiple_of(2) {
            ret[b] = d << 4;
        } else {
            ret[b] |= d;
        }
        j += 1;
    }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse::<4>(""), [0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_parse_single_nibble() {
        assert_eq!(parse::<1>("f"), [0x0f]);
        assert_eq!(parse::<4>("f"), [0x00, 0x00, 0x00, 0x0f]);
    }

    #[test]
    fn test_parse_single_byte() {
        assert_eq!(parse::<1>("ff"), [0xff]);
        assert_eq!(parse::<4>("ff"), [0x00, 0x00, 0x00, 0xff]);
    }

    #[test]
    fn test_parse_exact() {
        assert_eq!(parse::<4>("deadbeef"), [0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(parse::<4>("00000000"), [0x00, 0x00, 0x00, 0x00]);
        assert_eq!(parse::<4>("ffffffff"), [0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn test_parse_short() {
        assert_eq!(parse::<4>("beef"), [0x00, 0x00, 0xbe, 0xef]);
        assert_eq!(parse::<4>("1"), [0x00, 0x00, 0x00, 0x01]);
    }

    #[test]
    #[should_panic]
    fn test_parse_too_long() {
        let _ = parse::<4>("deadbeef00");
    }

    #[test]
    #[should_panic]
    fn test_parse_invalid_char() {
        let _ = parse::<4>("xyz0");
    }
}
