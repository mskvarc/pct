//! Byte-scanners that skip over "plain" (keep-as-is) runs during percent-encoding.
//!
//! A byte is a *break* if it is non-ASCII (`>= 0x80`), equals `%`, or its
//! entry in the encoder's 128-byte ASCII keep table is `0`. Everything else
//! is plain and can be copied verbatim. The scanners take the keep table as
//! a parameter so encoders with different reserved sets share the same code.

/// Build a 16-byte nibble-shuffle derived from a 128-entry ASCII keep table.
///
/// Each output byte `out[lo]` has bit `hi` set iff `table[(hi << 4) | lo] != 0`
/// for `hi` in `0..8`. Paired with a high-nibble shuffle mapping `hi` in `0..8`
/// to `1 << hi` (and `hi` in `8..16` to `0`), two `swizzle_dyn` lookups plus a
/// bitwise AND reproduce the full 128-entry lookup for any byte, with
/// non-ASCII bytes (hi >= 8) always landing on the drop side.
pub(crate) const fn build_lo_shuf(table: &[u8; 128]) -> [u8; 16] {
    let mut out = [0u8; 16];
    let mut lo = 0usize;
    while lo < 16 {
        let mut bits: u8 = 0;
        let mut hi = 0u8;
        while hi < 8 {
            if table[((hi as usize) << 4) | lo] != 0 {
                bits |= 1 << hi;
            }
            hi += 1;
        }
        out[lo] = bits;
        lo += 1;
    }
    out
}

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

/// 16-lane portable-SIMD prefilter. Same contract as [`scan_keep_run_swar`].
///
/// The SIMD pass only checks the two "class" conditions that can be expressed
/// as simple byte-vector comparisons — `byte >= 0x80` and `byte == b'%'`.
/// The per-encoder 128-entry keep table is still consulted scalarly, but only
/// for the lanes that made it past the prefilter and inside the lane that
/// triggered the break. In practice plain-heavy inputs stay in the 16-byte
/// stride and the scalar table lookups are loop-unrolled by the optimizer.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn scan_keep_run_simd(bytes: &[u8], mut i: usize, table: &[u8; 128]) -> usize {
    use core::simd::{
        Simd,
        cmp::{SimdPartialEq, SimdPartialOrd},
    };
    const LANES: usize = 16;

    while i + LANES <= bytes.len() {
        let v: Simd<u8, LANES> = Simd::from_slice(&bytes[i..i + LANES]);
        let non_ascii = v.simd_ge(Simd::splat(0x80));
        let is_pct = v.simd_eq(Simd::splat(b'%'));
        let break_bits = (non_ascii | is_pct).to_bitmask();
        if break_bits != 0 {
            let pct_pos = break_bits.trailing_zeros() as usize;
            // A table-declined ASCII byte before `pct_pos` still wins.
            for k in 0..pct_pos {
                if table[bytes[i + k] as usize] == 0 {
                    return i + k;
                }
            }
            return i + pct_pos;
        }
        // Prefilter passed: all lanes are ASCII and non-`%`. Check keep table.
        // Early-exit on the first declined lane: the return value is the
        // earliest break, which is exactly the first `k` whose lookup is 0.
        // A full-mask build would force 16 scalar lookups even when the first
        // lane is already a drop (catastrophic for break-heavy input).
        for k in 0..LANES {
            if table[bytes[i + k] as usize] == 0 {
                return i + k;
            }
        }
        i += LANES;
    }
    scan_keep_run_swar(bytes, i, table)
}

/// 16-lane SIMD scanner using a nibble-shuffle keep-table lookup.
///
/// Given `lo_shuf` as produced by [`build_lo_shuf`], two `swizzle_dyn` lookups
/// plus a bitwise AND produce a per-lane keep bit without any scalar lookups.
/// Non-ASCII bytes (hi nibble >= 8) always land on the drop side because the
/// high-nibble shuffle zeroes those positions, so `break_bits` captures
/// non-ASCII, `%` (encoded as a 0 in every keep table), and table-declined
/// bytes in a single pass.
#[cfg(feature = "simd")]
#[inline]
pub(crate) fn scan_keep_run_simd_shuf(bytes: &[u8], mut i: usize, table: &[u8; 128], lo_shuf: &[u8; 16]) -> usize {
    use core::simd::{Simd, cmp::SimdPartialEq};
    const LANES: usize = 16;

    let hi_lookup: Simd<u8, LANES> = Simd::from_array([1, 2, 4, 8, 16, 32, 64, 128, 0, 0, 0, 0, 0, 0, 0, 0]);
    let lo_lookup: Simd<u8, LANES> = Simd::from_array(*lo_shuf);
    let mask_0f: Simd<u8, LANES> = Simd::splat(0x0F);
    let shr_4: Simd<u8, LANES> = Simd::splat(4);
    let zero: Simd<u8, LANES> = Simd::splat(0);

    while i + LANES <= bytes.len() {
        let v: Simd<u8, LANES> = Simd::from_slice(&bytes[i..i + LANES]);
        let lo_nib = v & mask_0f;
        let hi_nib = v >> shr_4;
        let lo_bits = lo_lookup.swizzle_dyn(lo_nib);
        let hi_bits = hi_lookup.swizzle_dyn(hi_nib);
        let drop_bits = (lo_bits & hi_bits).simd_eq(zero).to_bitmask();
        if drop_bits != 0 {
            return i + drop_bits.trailing_zeros() as usize;
        }
        i += LANES;
    }
    scan_keep_run_swar(bytes, i, table)
}

/// Dispatch entry point. Routes to the nibble-shuffle SIMD variant when the
/// encoder ships a precomputed `lo_shuf`, to the prefilter SIMD variant
/// otherwise under `feature = "simd"`, and to SWAR when the feature is off.
#[inline]
pub(crate) fn scan_keep_run(bytes: &[u8], i: usize, table: &[u8; 128], lo_shuf: Option<&'static [u8; 16]>) -> usize {
    #[cfg(feature = "simd")]
    {
        match lo_shuf {
            Some(ls) => scan_keep_run_simd_shuf(bytes, i, table, ls),
            None => scan_keep_run_simd(bytes, i, table),
        }
    }
    #[cfg(not(feature = "simd"))]
    {
        let _ = lo_shuf;
        scan_keep_run_swar(bytes, i, table)
    }
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

    #[cfg(feature = "simd")]
    #[test]
    fn simd_matches_reference_random() {
        let table = &crate::encoder::uri::URI_KEEP_ANY;
        let mut state: u32 = 0x1234_5678;
        let mut bytes = vec![0u8; 1024];
        for b in bytes.iter_mut() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (state >> 8) as u8;
        }
        for start in 0..bytes.len() {
            let a = scan_keep_run_simd(&bytes, start, table);
            let b = scan_reference(&bytes, start, table);
            assert_eq!(a, b, "start={}", start);
        }
    }

    #[cfg(feature = "simd")]
    #[test]
    fn simd_shuf_matches_reference_random() {
        let table = &crate::encoder::uri::URI_KEEP_ANY;
        let lo_shuf = build_lo_shuf(table);
        let mut state: u32 = 0x1234_5678;
        let mut bytes = vec![0u8; 1024];
        for b in bytes.iter_mut() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (state >> 8) as u8;
        }
        for start in 0..bytes.len() {
            let a = scan_keep_run_simd_shuf(&bytes, start, table, &lo_shuf);
            let b = scan_reference(&bytes, start, table);
            assert_eq!(a, b, "start={}", start);
        }
    }

    #[test]
    fn lo_shuf_reproduces_table() {
        // Verify: for every ASCII byte, nibble-shuffle lookup == scalar table.
        let table = &crate::encoder::uri::URI_KEEP_ANY;
        let lo_shuf = build_lo_shuf(table);
        let hi_shuf: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 0, 0, 0, 0, 0, 0, 0, 0];
        for b in 0u8..=127 {
            let keep_shuf = (lo_shuf[(b & 0x0F) as usize] & hi_shuf[(b >> 4) as usize]) != 0;
            let keep_tbl = table[b as usize] != 0;
            assert_eq!(keep_shuf, keep_tbl, "byte {:#x}", b);
        }
        // Non-ASCII always drops via hi_shuf zeros.
        for b in 128u8..=255 {
            let keep_shuf = (lo_shuf[(b & 0x0F) as usize] & hi_shuf[(b >> 4) as usize]) != 0;
            assert!(!keep_shuf, "non-ASCII {:#x} must drop", b);
        }
    }
}
