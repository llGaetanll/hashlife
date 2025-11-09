use thiserror::Error;

use crate::parse_util;
use crate::parse_util::ParseError;

const NBHD_MASK: u16 = 0b0000_0111_0101_0111;
const CELL_MASK: u16 = 0b0000_0000_0010_0000;

// Count the bits using Brian Kernighan's way
// See: http://graphics.stanford.edu/~seander/bithacks.html#CountBitsSetKernighan
fn count_bits(mut x: u16) -> u8 {
    let mut n = 0;

    while x != 0 {
        x &= x - 1;
        n += 1;
    }

    n
}

/// Rules of Conway's Game of Life.
pub const B3S23: RuleSet = RuleSet::new(0b1000, 0b1100);

/// # Representation
/// Life rules are represented as
/// ```notrust
/// |------birth------|
/// 0000_0000_0000_0000_0000_0000_0000_0000
///                     |----survival-----|
/// ```
///
/// # Examples
/// ```notrust
/// b3s23:                0000_0000_0000_1000_0000_0000_0000_1100
///
/// b0s0:                 0000_0000_0000_0000_0000_0000_0000_0000
/// b012345678s012345678: 0000_0001_1111_1111_0000_0001_1111_1111
/// ```
///
/// See: https://conwaylife.com/wiki/Rulestring
#[derive(Clone)]
pub struct RuleSet {
    rule: u32,

    ext: Option<RuleExtension>,
}

impl Default for RuleSet {
    fn default() -> Self {
        B3S23
    }
}

impl std::fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut rule_str = String::from("b");

        // Birth rules (upper 16 bits)
        for i in 0..9 {
            if (self.rule >> (16 + i)) & 1 != 0 {
                rule_str.push((b'0' + i) as char);
            }
        }

        rule_str.push_str("/s");

        // Survival rules (lower 16 bits)
        for i in 0..9 {
            if (self.rule >> i) & 1 != 0 {
                rule_str.push((b'0' + i) as char);
            }
        }

        f.debug_struct("RuleSet")
            .field("rule", &rule_str)
            .field("ext", &self.ext)
            .finish()
    }
}

impl RuleSet {
    /// Create a new `RuleSet` for the given births and survivals. For both `b` and
    /// `s`, numbers are set on a bit basis. For instance if bit `i` in `b` is on, it
    /// means `i` is included in the set of births. Any bit past the 8th is ignored.
    ///
    /// Big endian is used here (i.e. `b = 0b1` means b1, and `b = 0b1_0000_0000` means b8).
    pub const fn new(b: u16, s: u16) -> Self {
        let b = b & 0x1FF;
        let s = s & 0x1FF;

        Self {
            rule: (b as u32) << 16 | s as u32,
            ext: None,
        }
    }

    pub const fn with_extension(b: u16, s: u16, ext: RuleExtension) -> Self {
        let b = b & 0x1FF;
        let s = s & 0x1FF;

        Self {
            rule: (b as u32) << 16 | s as u32,
            ext: Some(ext),
        }
    }

    pub fn births(&self) -> u16 {
        ((self.rule & 0x1FF0000) >> 0x10) as u16
    }

    pub fn survivals(&self) -> u16 {
        (self.rule & 0x1FF) as u16
    }

    pub fn extension(&self) -> Option<&RuleExtension> {
        self.ext.as_ref()
    }

    /// Compute game rules for the current `RuleSet`.
    ///
    /// More specifically, this returns a list of all
    /// possible 4x4 cells, each stored using the bits
    /// of a `u16`. These will eventually become the
    /// leaves in our world's quadtree.
    ///
    /// The array is built in a way that indexing into
    /// it with a certain rule will return the result
    /// of that rule.
    pub fn compute_rules(&self) -> Vec<u16> {
        let mut rules = vec![0; (u16::MAX as usize) + 1];

        for cell in 0..=u16::MAX {
            rules[cell as usize] = self.next(cell);
        }

        rules
    }

    fn next(&self, cell: u16) -> u16 {
        let mut res: u16 = 0;

        // goes: top right, top left, bot right, bot left
        let shifts = [0, 1, 4, 5];

        let births = self.births();
        let survivals = self.survivals();

        for shift in shifts {
            let nbhd_mask = NBHD_MASK << shift;
            let cell_mask = CELL_MASK << shift;

            let dead = (cell & cell_mask) == 0;
            let num_neighbors = count_bits(cell & nbhd_mask);

            let num_neighbors = 1 << num_neighbors;

            if dead {
                if num_neighbors as u16 & births == num_neighbors as u16 {
                    res |= cell_mask;
                }
            } else if num_neighbors as u16 & survivals == num_neighbors as u16 {
                res |= cell_mask;
            }
        }

        res
    }
}

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("Parse error: {0}")]
    ParseError(#[from] ParseError),

    #[error("Header rule must contain b or B")]
    NoBirths,

    #[error("Some number of births is required")]
    NoBirthsCount,

    #[error("Header rule must contain s or S")]
    NoSurvivals,

    #[error("Some number of survivals is required")]
    NoSurvivalsCount,

    #[error("Birth count should only contain digits")]
    BirthCountContainsNonDigits,

    #[error("Survival count should only contain digits")]
    SurvivalCountContainsNonDigits,

    #[error("Rule extension error: {0}")]
    ExtensionError(#[from] RuleExtensionError),
}

#[inline]
fn survival_stop_fn(b: u8) -> bool {
    b.is_ascii_whitespace() ||

    // Rule extensions
    b == b':'
}

// Parse rules that look like b3/s23
pub(crate) fn parse_rule(bytes: &[u8]) -> Result<(RuleSet, &[u8]), RuleError> {
    let (Some(b'b' | b'B'), bytes) = parse_util::take_1(bytes) else {
        return Err(RuleError::NoBirths);
    };

    let (Some(b), bytes) = parse_util::take_until(b'/', bytes) else {
        return Err(RuleError::NoBirthsCount);
    };
    let b = bytes_to_num(b).map_err(|_| RuleError::BirthCountContainsNonDigits)?;

    let bytes = parse_util::expect(b'/', bytes)?;

    let (Some(b's' | b'S'), bytes) = parse_util::take_1(bytes) else {
        return Err(RuleError::NoSurvivals);
    };

    let (Some(s), bytes) = parse_util::take_until_fn(survival_stop_fn, bytes) else {
        return Err(RuleError::NoSurvivalsCount);
    };
    let s = bytes_to_num(s).map_err(|_| RuleError::SurvivalCountContainsNonDigits)?;

    let (rule, bytes) = if let Some(b':') = parse_util::peek_1(bytes) {
        let (ext, bytes) = parse_rule_extension(bytes)?;

        (RuleSet::with_extension(b, s, ext), bytes)
    } else {
        (RuleSet::new(b, s), bytes)
    };

    Ok((rule, bytes))
}

// Parse rules that look like 3/23. These show up in RLE #r comment lines.
pub(crate) fn parse_nameless_rule(bytes: &[u8]) -> Result<(RuleSet, &[u8]), RuleError> {
    let (Some(b), bytes) = parse_util::take_until(b'/', bytes) else {
        return Err(RuleError::NoBirthsCount);
    };
    let b = bytes_to_num(b).map_err(|_| RuleError::BirthCountContainsNonDigits)?;

    let bytes = parse_util::expect(b'/', bytes)?;

    let (Some(s), bytes) = parse_util::take_until_fn(survival_stop_fn, bytes) else {
        return Err(RuleError::NoSurvivalsCount);
    };
    let s = bytes_to_num(s).map_err(|_| RuleError::SurvivalCountContainsNonDigits)?;

    let (rule, bytes) = if let Some(b':') = parse_util::peek_1(bytes) {
        let (ext, bytes) = parse_rule_extension(bytes)?;

        (RuleSet::with_extension(b, s, ext), bytes)
    } else {
        (RuleSet::new(b, s), bytes)
    };

    Ok((rule, bytes))
}

#[derive(Debug, Clone, Copy)]
pub enum RuleTopology {
    Planar,
    Torus,
    KleinBottle,
    Spherical,
    Cylindrical,
}

#[derive(Debug, Clone, Copy)]
pub enum RuleSize {
    Bounded(u32),
    Unbounded,
}

#[derive(Debug, Clone)]
pub struct RuleExtension {
    pub topology: RuleTopology,
    pub width: RuleSize,
    pub height: RuleSize,
}

#[derive(Debug, Error)]
pub enum RuleExtensionError {
    #[error("Parse error")]
    ParseError(#[from] ParseError),

    #[error("Unexpected EOF")]
    UnexpectedEof,

    #[error("Unrecognized topology: '{got}'")]
    UnrecognizedTopology { got: char },

    #[error("Width undefined")]
    NoWidth,

    #[error("Height undefined")]
    NoHeight,

    #[error("Failed to parse width. Should be either <number> or *, got: {got}")]
    ParseWidth { got: String },

    #[error("Failed to parse height. Should be either <number> or *, got: {got}")]
    ParseHeight { got: String },
}

pub(crate) fn parse_rule_extension(
    bytes: &[u8],
) -> Result<(RuleExtension, &[u8]), RuleExtensionError> {
    let bytes = parse_util::expect(b':', bytes)?;

    let (Some(b), bytes) = parse_util::take_1(bytes) else {
        return Err(RuleExtensionError::UnexpectedEof);
    };

    let topology = match b {
        b'P' => RuleTopology::Planar,

        b'T' => RuleTopology::Torus,

        b'K' => RuleTopology::KleinBottle,

        b'S' => RuleTopology::Spherical,

        b'C' => RuleTopology::Cylindrical,

        b => return Err(RuleExtensionError::UnrecognizedTopology { got: b as char }),
    };

    let (Some(width_bs), bytes) = parse_util::take_until(b',', bytes) else {
        return Err(RuleExtensionError::NoWidth);
    };

    let width = match parse_util::convert(width_bs) {
        Ok(width) => RuleSize::Bounded(width),
        Err(_) => {
            if parse_util::is(b'*', width_bs).is_ok() {
                RuleSize::Unbounded
            } else {
                return Err(RuleExtensionError::ParseWidth {
                    got: String::from_utf8_lossy(width_bs).to_string(),
                });
            }
        }
    };

    let bytes = parse_util::expect(b',', bytes)?;

    let (Some(height_bs), bytes) = parse_util::take_until_ws(bytes) else {
        return Err(RuleExtensionError::NoHeight);
    };

    let height = match parse_util::convert(height_bs) {
        Ok(height) => RuleSize::Bounded(height),
        Err(_) => {
            if parse_util::is(b'*', height_bs).is_ok() {
                RuleSize::Unbounded
            } else {
                return Err(RuleExtensionError::ParseHeight {
                    got: String::from_utf8_lossy(height_bs).to_string(),
                });
            }
        }
    };

    let extension = RuleExtension {
        topology,
        width,
        height,
    };

    Ok((extension, bytes))
}

/// Convert the human readable birth/survival number to a packed bit representation
fn bytes_to_num(bytes: &[u8]) -> Result<u16, ()> {
    let mut n = 0;

    for &b in bytes {
        if !b.is_ascii_digit() {
            return Err(());
        }

        n |= 1 << (b - b'0');
    }

    Ok(n)
}

#[cfg(test)]
mod tests {
    use crate::rule_set::RuleError;

    #[test]
    fn test_rule_with_extension() -> Result<(), RuleError> {
        // NOTE: final whitespace needed since RLE files never end in a rule
        let rule_bs = b"B3/S23:T100,58 ";

        let (rule, bs) = super::parse_rule(rule_bs)?;

        insta::assert_debug_snapshot!(rule, @r#"
            RuleSet {
                rule: "b3/s23",
                ext: Some(
                    RuleExtension {
                        topology: Torus,
                        width: Bounded(
                            100,
                        ),
                        height: Bounded(
                            58,
                        ),
                    },
                ),
            }
        "#);
        assert_eq!(bs, b" ");

        Ok(())
    }

    #[test]
    fn test_rule_with_unbounded_extension() -> Result<(), RuleError> {
        // NOTE: final whitespace needed since RLE files never end in a rule
        let rule_bs = b"B3/S23:T100,* ";

        let (rule, bs) = super::parse_rule(rule_bs)?;

        insta::assert_debug_snapshot!(rule, @r#"
            RuleSet {
                rule: "b3/s23",
                ext: Some(
                    RuleExtension {
                        topology: Torus,
                        width: Bounded(
                            100,
                        ),
                        height: Unbounded,
                    },
                ),
            }
        "#);
        assert_eq!(bs, b" ");

        Ok(())
    }
}
