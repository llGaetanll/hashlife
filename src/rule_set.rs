use anyhow::bail;
use anyhow::Context;

use crate::parse_util;

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
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct RuleSet {
    rule: u32,
}

impl Default for RuleSet {
    fn default() -> Self {
        B3S23
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
        }
    }

    pub fn births(&self) -> u16 {
        ((self.rule & 0x1FF0000) >> 0x10) as u16
    }

    pub fn survivals(&self) -> u16 {
        (self.rule & 0x1FF) as u16
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

// Parse rules that look like b3/s23
pub(crate) fn parse_rule(bytes: &[u8]) -> parse_util::ParseResult<(RuleSet, &[u8])> {
    let (Some(b'b' | b'B'), bytes) = parse_util::take_1(bytes) else {
        bail!("Header rule contains b or B")
    };

    let (Some(b), bytes) = parse_util::take_until(b'/', bytes) else {
        bail!("Some number of births is required")
    };

    let bytes = parse_util::expect(b'/', bytes)?;

    let (Some(b's' | b'S'), bytes) = parse_util::take_1(bytes) else {
        bail!("Header rule contains s or S")
    };

    let (Some(s), bytes) = parse_util::take_until_ws(bytes) else {
        bail!("Some number of births is required")
    };

    let b = bytes_to_num(b).context("Failed to convert births")?;
    let s = bytes_to_num(s).context("Failed to convert survivals")?;

    Ok((RuleSet::new(b, s), bytes))
}

// Parse rules that look like 3/23. These show up in RLE #r comment lines.
pub(crate) fn parse_nameless_rule(bytes: &[u8]) -> parse_util::ParseResult<(RuleSet, &[u8])> {
    let (Some(b), bytes) = parse_util::take_until(b'/', bytes) else {
        bail!("Some number of births is required")
    };

    let bytes = parse_util::expect(b'/', bytes)?;

    let (Some(s), bytes) = parse_util::take_until_ws(bytes) else {
        bail!("Some number of births is required")
    };

    let b = bytes_to_num(b).context("Failed to convert births")?;
    let s = bytes_to_num(s).context("Failed to convert survivals")?;

    Ok((RuleSet::new(b, s), bytes))
}

/// Convert the human readable birth/survival number to a packed bit representation
fn bytes_to_num(bytes: &[u8]) -> anyhow::Result<u16> {
    let mut n = 0;

    for &b in bytes {
        if !b.is_ascii_digit() {
            bail!("expected digits only")
        }

        n |= 1 << (b - b'0');
    }

    Ok(n)
}
