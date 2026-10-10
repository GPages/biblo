//! Code 128 encoder, code set B only (printable ASCII). Our codes are short
//! (`L000123`), so the shorter code set C is not worth the extra logic.
//!
//! Output is the list of modules (narrowest bar or space units), `true` = bar.
//! The quiet zone (blank margin on each side) is NOT included: the caller
//! leaves at least `QUIET_ZONE` blank modules on the left and right.

/// Minimum blank margin on each side, in modules (the standard asks for 10).
pub const QUIET_ZONE: usize = 10;

const START_B: usize = 104;
const STOP: &str = "2331112";

/// Bar/space widths of symbols 0..=105, from the Code 128 standard table.
/// Each symbol is bar, space, bar, space, bar, space, 11 modules in total.
const PATTERNS: [&str; 106] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", // 0
    "132212", "221213", "221312", "231212", "112232", "122132", "122231", "113222", // 8
    "123122", "123221", "223211", "221132", "221231", "213212", "223112", "312131", // 16
    "311222", "321122", "321221", "312212", "322112", "322211", "212123", "212321", // 24
    "232121", "111323", "131123", "131321", "112313", "132113", "132311", "211313", // 32
    "231113", "231311", "112133", "112331", "132131", "113123", "113321", "133121", // 40
    "313121", "211331", "231131", "213113", "213311", "213131", "311123", "311321", // 48
    "331121", "312113", "312311", "332111", "314111", "221411", "431111", "111224", // 56
    "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114", // 64
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", // 72
    "111242", "121142", "121241", "114212", "124112", "124211", "411212", "421112", // 80
    "421211", "212141", "214121", "412121", "111143", "111341", "131141", "114113", // 88
    "114311", "411113", "411311", "113141", "114131", "311141", "411131", "211412", // 96
    "211214", "211232", // 104 = Start B, 105 = Start C
];

fn push_pattern(modules: &mut Vec<bool>, widths: &str) {
    for (i, w) in widths.bytes().enumerate() {
        let is_bar = i % 2 == 0;
        for _ in 0..(w - b'0') {
            modules.push(is_bar);
        }
    }
}

/// Encodes `text`. Returns `None` if it is empty or contains a character
/// outside printable ASCII (space to `~`).
pub fn encode(text: &str) -> Option<Vec<bool>> {
    if text.is_empty() || !text.bytes().all(|b| (32..=126).contains(&b)) {
        return None;
    }
    let values: Vec<usize> = text.bytes().map(|b| (b - 32) as usize).collect();
    let checksum = values
        .iter()
        .enumerate()
        .fold(START_B, |sum, (i, v)| sum + (i + 1) * v)
        % 103;

    let mut modules = Vec::with_capacity(11 * (values.len() + 2) + 13);
    push_pattern(&mut modules, PATTERNS[START_B]);
    for &v in &values {
        push_pattern(&mut modules, PATTERNS[v]);
    }
    push_pattern(&mut modules, PATTERNS[checksum]);
    push_pattern(&mut modules, STOP);
    Some(modules)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(modules: &[bool]) -> String {
        modules.iter().map(|&b| if b { '1' } else { '0' }).collect()
    }

    fn sum(widths: &str) -> u32 {
        widths.bytes().map(|w| (w - b'0') as u32).sum()
    }

    #[test]
    fn every_symbol_is_11_modules_and_stop_is_13() {
        for (i, p) in PATTERNS.iter().enumerate() {
            assert_eq!(p.len(), 6, "symbol {i}");
            assert_eq!(sum(p), 11, "symbol {i}");
        }
        assert_eq!(sum(STOP), 13);
    }

    #[test]
    fn patterns_are_unique() {
        let mut sorted = PATTERNS.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), PATTERNS.len());
    }

    #[test]
    fn length_and_edges() {
        let m = encode("L000123").unwrap();
        // start + 7 characters + checksum = 9 symbols, plus the stop pattern
        assert_eq!(m.len(), 9 * 11 + 13);
        // starts with a bar, ends with the stop's final bar
        assert!(m[0]);
        assert!(m[m.len() - 1] && m[m.len() - 2]);
    }

    #[test]
    fn checksums_computed_by_hand() {
        // L000123: (104 + 1*44 + 2*16 + 3*16 + 4*16 + 5*17 + 6*18 + 7*19) % 103 = 618 % 103 = 0
        // The checksum is the 9th symbol (modules 88..99). Symbol 0 is "212222".
        let m = encode("L000123").unwrap();
        assert_eq!(bits(&m[88..99]), "11011001100");

        // E000045: (104 + 37 + 32 + 48 + 64 + 80 + 120 + 147) % 103 = 632 % 103 = 14
        // Symbol 14 is "122231".
        let m = encode("E000045").unwrap();
        assert_eq!(bits(&m[88..99]), "10011001110");
    }

    #[test]
    fn matches_independent_implementation() {
        // Produced by python-barcode 0.16.1 (`barcode.get("code128", "Wikipedia").build()`),
        // which also uses code set B for text without digits.
        let expected = concat!(
            "11010010000",
            "11101000110",
            "10000110100",
            "11000010010",
            "10000110100",
            "10100111100",
            "10110010000",
            "10000100110",
            "10000110100",
            "10010110000",
            "11110010010",
            "1100011101011"
        );
        assert_eq!(bits(&encode("Wikipedia").unwrap()), expected);
    }

    #[test]
    fn rejects_unencodable_text() {
        assert_eq!(encode(""), None);
        assert_eq!(encode("é"), None);
        assert_eq!(encode("a\tb"), None);
    }
}
