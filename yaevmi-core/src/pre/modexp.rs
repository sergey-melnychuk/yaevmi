use num_bigint::BigUint;
use num_traits::{One, Zero};

/// MODEXP (precompile 0x05) - EIP-198 / EIP-2565
/// Input: Bsize (32) || Esize (32) || Msize (32) || B (Bsize) || E (Esize) || M (Msize)
/// Output: result zero-padded to Msize bytes
pub fn modexp(input: &[u8], gas_limit: i64) -> (bool, Vec<u8>, i64) {
    // Pad input to at least 96 bytes for the three length fields
    let mut buf = input.to_vec();
    if buf.len() < 96 {
        buf.resize(96, 0);
    }

    let b_size = read_u64(&buf[0..32]) as usize;
    let e_size = read_u64(&buf[32..64]) as usize;
    let m_size = read_u64(&buf[64..96]) as usize;

    // Protect against absurd sizes
    if b_size > 8192 || e_size > 8192 || m_size > 8192 {
        return (false, vec![], gas_limit);
    }

    // Gas calculation per EIP-2565
    let gas = modexp_gas(b_size, e_size, m_size, &buf);
    if gas > gas_limit {
        return (false, vec![], gas_limit);
    }

    if m_size == 0 {
        return (true, vec![], gas);
    }

    // Extract B, E, M from input (zero-pad if input is short)
    let data = &buf[96..];
    let b_bytes = safe_slice(data, 0, b_size);
    let e_bytes = safe_slice(data, b_size, e_size);
    let m_bytes = safe_slice(data, b_size + e_size, m_size);

    let base = BigUint::from_bytes_be(&b_bytes);
    let exp = BigUint::from_bytes_be(&e_bytes);
    let modulus = BigUint::from_bytes_be(&m_bytes);

    let result = if modulus.is_zero() || modulus.is_one() {
        BigUint::zero()
    } else {
        base.modpow(&exp, &modulus)
    };

    let result_bytes = result.to_bytes_be();
    let mut out = vec![0u8; m_size];
    if result_bytes.len() <= m_size {
        out[m_size - result_bytes.len()..].copy_from_slice(&result_bytes);
    } else {
        out.copy_from_slice(&result_bytes[result_bytes.len() - m_size..]);
    }

    (true, out, gas)
}

fn safe_slice(data: &[u8], offset: usize, size: usize) -> Vec<u8> {
    let mut result = vec![0u8; size];
    let lo = offset.min(data.len());
    let hi = (offset + size).min(data.len());
    let len = hi - lo;
    if len > 0 {
        result[..len].copy_from_slice(&data[lo..hi]);
    }
    result
}

fn read_u64(data: &[u8]) -> u64 {
    // Read big-endian 256-bit, but clamp to u64
    // Only the last 8 bytes matter if the value fits in u64
    let bytes: [u8; 32] = data[..32].try_into().unwrap();
    // Check if any of the high bytes are non-zero
    if bytes[..24].iter().any(|&b| b != 0) {
        return u64::MAX; // overflow
    }
    u64::from_be_bytes(bytes[24..32].try_into().unwrap())
}

fn modexp_gas(b_size: usize, e_size: usize, m_size: usize, input: &[u8]) -> i64 {
    // EIP-2565 gas calculation
    let max_len = b_size.max(m_size) as u64;
    let words = max_len.div_ceil(8);
    let multiplication_complexity = words * words;

    let iteration_count = calc_iteration_count(e_size, input);

    let cost = (multiplication_complexity * iteration_count.max(1)) / 3;
    cost.max(200) as i64
}

fn calc_iteration_count(e_size: usize, input: &[u8]) -> u64 {
    let data = &input[96..];
    let b_size = read_u64(&input[0..32]) as usize;
    let e_start = 96 + b_size;

    if e_size <= 32 {
        let e_bytes = safe_slice(data, b_size, e_size);
        let e = BigUint::from_bytes_be(&e_bytes);
        if e.is_zero() {
            return 0;
        }
        let bits = e.bits() as u64;
        bits - 1
    } else {
        // Get the first 32 bytes of the exponent
        let first_32 = safe_slice(
            if e_start < input.len() {
                &input[e_start..]
            } else {
                &[]
            },
            0,
            32.min(e_size),
        );
        let e_head = BigUint::from_bytes_be(&first_32);
        let head_bits = if e_head.is_zero() {
            0u64
        } else {
            e_head.bits() as u64 - 1
        };
        head_bits + 8 * (e_size as u64 - 32)
    }
}
