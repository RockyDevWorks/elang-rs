//! Normalization Form C (NFC) over pinned UCD 18.0.0 data.
//!
//! Implements the UAX#15 algorithm — canonical decomposition, canonical
//! ordering by Canonical_Combining_Class, then canonical composition — using
//! the tables generated from the pinned UCD 18.0.0 files
//! (the private `crate::nfc_tables` module). Hangul syllables are handled by the algorithmic
//! rules in UAX#15 §3.12 rather than table lookups.
//!
//! Correctness is demonstrated by the official Unicode 18.0.0
//! `NormalizationTest.txt` conformance suite (`tests/normalization_conformance.rs`),
//! which is the gate for claiming NFC 18.0.0 compliance (ADR 0007).
//!
//! Lint note: this module performs deliberate, bounds-guarded code-point
//! arithmetic (Hangul index math per UAX#15 §3.12) and indexes into tables at
//! positions returned by `binary_search` and into runs bounded by their own
//! length. These are the normal shape of a normalization kernel; the
//! workspace's untrusted-input lints (which target *accidental* unchecked math)
//! are relaxed here, and the whole module's correctness is pinned by the
//! official 18.0.0 conformance suite rather than by those lints.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use crate::nfc_tables::{CCC, COMPOSE, DECOMP};

// Hangul constants (UAX#15 §3.12).
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT; // 588
const S_COUNT: u32 = L_COUNT * N_COUNT; // 11172

/// Canonical_Combining_Class of a scalar (0 if not in the table).
#[must_use]
pub fn ccc(c: char) -> u8 {
    let cp = c as u32;
    match CCC.binary_search_by(|&(k, _)| k.cmp(&cp)) {
        Ok(i) => CCC[i].1,
        Err(_) => 0,
    }
}

fn canonical_decomp(cp: u32) -> Option<&'static [u32]> {
    match DECOMP.binary_search_by(|&(k, _)| k.cmp(&cp)) {
        Ok(i) => Some(DECOMP[i].1),
        Err(_) => None,
    }
}

fn compose_pair(a: u32, b: u32) -> Option<u32> {
    match COMPOSE.binary_search_by(|&(k, _)| k.cmp(&(a, b))) {
        Ok(i) => Some(COMPOSE[i].1),
        Err(_) => None,
    }
}

fn is_hangul_syllable(cp: u32) -> bool {
    (S_BASE..S_BASE + S_COUNT).contains(&cp)
}

fn decompose_hangul(cp: u32, out: &mut Vec<u32>) {
    let s_index = cp - S_BASE;
    let l = L_BASE + s_index / N_COUNT;
    let v = V_BASE + (s_index % N_COUNT) / T_COUNT;
    let t = T_BASE + s_index % T_COUNT;
    out.push(l);
    out.push(v);
    if t != T_BASE {
        out.push(t);
    }
}

/// Fully canonically decompose one scalar into `out` (recursive via the
/// precomputed full-decomposition table; Hangul handled algorithmically).
fn decompose_char(cp: u32, out: &mut Vec<u32>) {
    if is_hangul_syllable(cp) {
        decompose_hangul(cp, out);
        return;
    }
    match canonical_decomp(cp) {
        Some(seq) => out.extend_from_slice(seq),
        None => out.push(cp),
    }
}

/// Fully canonically decompose one scalar using pinned UCD 18.0.0 data.
/// Includes algorithmic Hangul decomposition; compatibility mappings are not
/// applied. This is for structural analysis without rewriting authored NFC.
#[must_use]
pub fn canonical_decomposition(scalar: char) -> Vec<char> {
    let mut decomposed = Vec::new();
    decompose_char(scalar as u32, &mut decomposed);
    decomposed.into_iter().filter_map(char::from_u32).collect()
}

/// Canonical ordering: stable sort of each maximal run of non-starters
/// (CCC != 0) by CCC (UAX#15 D109). Starters (CCC 0) are fixed points.
fn canonical_order(scalars: &mut [u32]) {
    let n = scalars.len();
    let mut i = 0;
    while i < n {
        if ccc_of(scalars[i]) == 0 {
            i += 1;
            continue;
        }
        // find the run [i, j) of non-starters
        let mut j = i;
        while j < n && ccc_of(scalars[j]) != 0 {
            j += 1;
        }
        // stable insertion sort by CCC over scalars[i..j]
        let run = &mut scalars[i..j];
        for a in 1..run.len() {
            let mut b = a;
            while b > 0 && ccc_of(run[b - 1]) > ccc_of(run[b]) {
                run.swap(b - 1, b);
                b -= 1;
            }
        }
        i = j;
    }
}

fn ccc_of(cp: u32) -> u8 {
    match CCC.binary_search_by(|&(k, _)| k.cmp(&cp)) {
        Ok(i) => CCC[i].1,
        Err(_) => 0,
    }
}

/// Canonical composition (UAX#15, the Canonical Composition Algorithm),
/// including algorithmic Hangul composition.
///
/// Scan left to right tracking the last starter L (CCC 0). A later character C
/// composes with L iff (a) L+C is a primary canonical composite and (b) C is
/// not *blocked*: C is blocked if the character immediately before it in the
/// sequence is also a non-starter with CCC >= CCC(C) (i.e. there is an
/// intervening equal-or-higher combining class), or if a starter intervened.
fn compose(scalars: &[u32]) -> Vec<u32> {
    if scalars.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<u32> = Vec::with_capacity(scalars.len());
    let mut last_starter: Option<usize> = None; // index into `out`
                                                // CCC of the last character appended *after* the current starter; 0 means
                                                // none yet. Used to detect blocking.
    let mut prev_ccc: u8 = 0;

    for &cp in scalars {
        let cur_ccc = ccc_of(cp);

        if let Some(sp) = last_starter {
            // C is combinable with the starter unless blocked. Blocking: a
            // preceding non-starter has CCC >= CCC(C) (for a non-starter C), or
            // for a starter C (cur_ccc==0) it is combinable only if no
            // non-starter has intervened since the starter (prev_ccc == 0).
            let blocked = if cur_ccc == 0 {
                prev_ccc != 0
            } else {
                prev_ccc != 0 && prev_ccc >= cur_ccc
            };
            if !blocked {
                if let Some(composed) = try_compose(out[sp], cp) {
                    out[sp] = composed;
                    // The composite is a starter; prev_ccc unchanged because no
                    // new character is appended between starter and the next.
                    continue;
                }
            }
        }

        out.push(cp);
        if cur_ccc == 0 {
            last_starter = Some(out.len() - 1);
            prev_ccc = 0;
        } else {
            prev_ccc = cur_ccc;
        }
    }
    out
}

fn try_compose(a: u32, b: u32) -> Option<u32> {
    // Algorithmic Hangul composition first (UAX#15 §3.12).
    // L + V
    if (L_BASE..L_BASE + L_COUNT).contains(&a) && (V_BASE..V_BASE + V_COUNT).contains(&b) {
        let l_index = a - L_BASE;
        let v_index = b - V_BASE;
        return Some(S_BASE + (l_index * V_COUNT + v_index) * T_COUNT);
    }
    // LV + T
    if is_hangul_syllable(a) {
        let s_index = a - S_BASE;
        if s_index.is_multiple_of(T_COUNT) && (T_BASE + 1..T_BASE + T_COUNT).contains(&b) {
            let t_index = b - T_BASE;
            return Some(a + t_index);
        }
    }
    compose_pair(a, b)
}

/// Return the NFC form of `s`.
#[must_use]
pub fn nfc(s: &str) -> String {
    // 1. full canonical decomposition
    let mut decomposed: Vec<u32> = Vec::with_capacity(s.len());
    for c in s.chars() {
        decompose_char(c as u32, &mut decomposed);
    }
    // 2. canonical ordering
    canonical_order(&mut decomposed);
    // 3. canonical composition
    let composed = compose(&decomposed);
    // 4. back to String (all values are valid scalars by construction)
    composed.into_iter().filter_map(char::from_u32).collect()
}

/// Is `s` already in NFC?
#[must_use]
pub fn is_nfc(s: &str) -> bool {
    nfc(s) == s
}

#[cfg(test)]
mod tests {
    use super::canonical_decomposition;

    #[test]
    fn canonical_decomposition_uses_full_pinned_mappings() {
        for (scalar, expected) in [
            ('ç', "c\u{0327}"),
            ('\u{1E09}', "c\u{0327}\u{0301}"),
            ('\u{212B}', "A\u{030A}"),
            ('\u{AC01}', "\u{1100}\u{1161}\u{11A8}"),
            ('\u{FB03}', "\u{FB03}"),
            ('\u{02B0}', "\u{02B0}"),
            ('t', "t"),
        ] {
            assert_eq!(
                canonical_decomposition(scalar),
                expected.chars().collect::<Vec<_>>()
            );
        }
    }
}
