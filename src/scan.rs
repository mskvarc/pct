//! Byte-scanners that skip over "plain" (keep-as-is) runs during percent-encoding.
//!
//! A byte is a *break* if it is non-ASCII (`>= 0x80`), equals `%`, or its
//! entry in the encoder's 128-byte ASCII keep table is `0`. Everything else
//! is plain and can be copied verbatim. The scanners take the keep table as
//! a parameter so encoders with different reserved sets share the same code.

/// 8-byte unrolled SWAR scanner. Returns the index of the first break byte
/// at or after `i`, or `bytes.len()` if none.
#[inline]
pub(crate) fn scan_keep_run_swar(bytes: &[u8], mut i: usize, table: &[u8; 128]) -> usize {
    while i + 8 <= bytes.len() {
        let chunk: [u8; 8] = bytes[i..i + 8].try_into().unwrap();
        let mut mask: u32 = 0;
        for k in 0..8 {
            let b = chunk[k];
            // `%` is already represented as a 0 entry in every keep table, but
            // keep the explicit test so the scanner works with any table.
            if b >= 0x80 || b == b'%' || table[b as usize] == 0 {
                mask |= 1 << k;
            }
        }
        if mask == 0 {
            i += 8;
        } else {
            return i + mask.trailing_zeros() as usize;
        }
    }
    while i < bytes.len() {
        let b = bytes[i];
        if b >= 0x80 || b == b'%' || table[b as usize] == 0 {
            break;
        }
        i += 1;
    }
    i
}

/// Dispatch entry point — identical to the SWAR variant for now; Phase 4
/// swaps in a SIMD implementation under `cfg(feature = "simd")`.
#[inline]
pub(crate) fn scan_keep_run(bytes: &[u8], i: usize, table: &[u8; 128]) -> usize {
    scan_keep_run_swar(bytes, i, table)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference scalar scanner; shared correctness oracle.
    fn scan_reference(bytes: &[u8], mut i: usize, table: &[u8; 128]) -> usize {
        while i < bytes.len() {
            let b = bytes[i];
            if b >= 0x80 || b == b'%' || table[b as usize] == 0 {
                break;
            }
            i += 1;
        }
        i
    }

    #[test]
    fn swar_matches_reference_random() {
        // URI `Any` table: alphanumerics + '-._~' kept.
        let table = &crate::encoder::uri::URI_KEEP_ANY;
        let mut state: u32 = 0x1234_5678;
        let mut bytes = vec![0u8; 1024];
        for b in bytes.iter_mut() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (state >> 8) as u8;
        }
        for start in 0..bytes.len() {
            let a = scan_keep_run_swar(&bytes, start, table);
            let b = scan_reference(&bytes, start, table);
            assert_eq!(a, b, "start={}", start);
        }
    }

    #[test]
    fn swar_all_plain() {
        let table = &crate::encoder::uri::URI_KEEP_ANY;
        let bytes: Vec<u8> = (b'a'..=b'z').cycle().take(128).collect();
        assert_eq!(scan_keep_run_swar(&bytes, 0, table), bytes.len());
    }

    #[test]
    fn swar_stops_on_percent() {
        let table = &crate::encoder::uri::URI_KEEP_ANY;
        let bytes = b"abcdefgh%xyz";
        assert_eq!(scan_keep_run_swar(bytes, 0, table), 8);
    }

    #[test]
    fn swar_stops_on_non_ascii() {
        let table = &crate::encoder::uri::URI_KEEP_ANY;
        let bytes = b"abc\xC3\xA9xyz";
        assert_eq!(scan_keep_run_swar(bytes, 0, table), 3);
    }
}
