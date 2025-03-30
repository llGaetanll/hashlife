use std::str::FromStr;

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
/// # Format
///
/// `b[0-8]{1,9}s[0-8]{1,9}`
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct RuleSet {
    rule: u32,
}

impl RuleSet {
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

    fn births(&self) -> u16 {
        ((self.rule & ((u16::MAX as u32) << 0x10)) >> 0x10) as u16
    }

    fn survivals(&self) -> u16 {
        (self.rule & u16::MAX as u32) as u16
    }
}

#[derive(Debug)]
pub enum RuleSetError {
    InvalidString,
}

impl FromStr for RuleSet {
    type Err = RuleSetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        enum State {
            Birth,
            Survival,
        }

        let mut state = State::Birth;
        let mut rule = 0;

        for c in s.chars() {
            match c {
                'b' | 'B' => {
                    state = State::Birth;
                }
                's' | 'S' => {
                    state = State::Survival;
                }
                n => {
                    let n = n.to_digit(10).ok_or(RuleSetError::InvalidString)? as u8;

                    if n > 8 {
                        return Err(RuleSetError::InvalidString);
                    }

                    match state {
                        State::Survival => {
                            rule |= 1 << n;
                        }
                        State::Birth => {
                            rule |= 1 << (n + 0x10);
                        }
                    }
                }
            }
        }

        Ok(RuleSet { rule })
    }
}
