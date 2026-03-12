pub trait RleBufWrite {
    /// Push `n` live cells to the data buffer
    fn live_cell(&mut self, n: u64);

    /// Push `n` dead cells to the data buffer
    fn dead_cell(&mut self, n: u64);

    /// Push `n` line breaks to the data buffer
    fn line_break(&mut self, n: u64);

    /// Push an EOF marker to the data buffer
    fn eof(&mut self);
}

/// A binary representation of an RLE file.
///
/// Internally stored as a buffer of bytes, numbers are flagged with the most significant bit as
/// zero, and magics with the most significant bit as one. All numbers are stored in big endian.
///
/// Because of this flag, the maximum possible run-length is 2^{63} - 1.
pub struct RleBuffer {
    hand: Option<(u8, u64)>,
    data: Vec<u8>,
}

pub enum RleBufferEntry {
    RunLength(u64),
    LiveCell,
    DeadCell,
    LineBreak,
}

const NUMER_MASK: u8 = 0x7F;
const MAGIC_MASK: u8 = 0x80;
const DEAD_CELL_MAGIC: u8 = 0b00;
const LIVE_CELL_MAGIC: u8 = 0b01;
const LINE_BREAK_MAGIC: u8 = 0b10;
const EOF_MAGIC: u8 = 0b11;

impl RleBufWrite for RleBuffer {
    fn live_cell(&mut self, n: u64) {
        self.insert(LIVE_CELL_MAGIC, n);
    }

    fn dead_cell(&mut self, n: u64) {
        self.insert(DEAD_CELL_MAGIC, n);
    }

    fn line_break(&mut self, n: u64) {
        self.insert(LINE_BREAK_MAGIC, n);
    }

    fn eof(&mut self) {
        if let Some((magic, n)) = self.hand {
            self.insert_buffer(magic, n);
            self.hand = None;
        }

        self.insert_magic(EOF_MAGIC);
    }
}

impl RleBuffer {
    pub fn new() -> Self {
        Self {
            hand: None,
            data: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            hand: None,
            data: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    fn insert(&mut self, magic: u8, n: u64) {
        match self.hand.as_mut() {
            None => self.hand = Some((magic, n)),
            Some((mag, num)) if *mag == magic => {
                *num += n;
            }
            Some((mag, num)) => {
                let (mm, nn) = (*mag, *num);
                (*mag, *num) = (magic, n);
                self.insert_buffer(mm, nn);
            }
        }
    }

    #[inline]
    fn insert_buffer(&mut self, magic: u8, n: u64) {
        if n > 1 {
            self.insert_number(n);
        }

        self.insert_magic(magic);
    }

    #[inline]
    fn insert_number(&mut self, n: u64) {
        // 9 bytes * 7 bits = 63 bits max
        debug_assert!(n <= (1 << 63) - 1, "Run length {n} exceeds max (2^63 - 1)");

        // Encode as big-endian, 7 bits per byte, MSB=0 on every byte.
        // Determine how many 7-bit groups we need (at least 1).
        let bits = 64 - n.leading_zeros().max(1) as u64; // bits needed
        let num_bytes = ((bits + 6) / 7) as u32; // ceil(bits / 7)

        for i in (0..num_bytes).rev() {
            let byte = ((n >> (i * 7)) & 0x7F) as u8;
            self.data.push(byte);
        }
    }

    #[inline]
    fn insert_magic(&mut self, magic: u8) {
        self.data.push(magic | MAGIC_MASK);
    }
}

impl Default for RleBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// A type that implements `RleBuf` where every operation is a noop. Useful for parsing tests
pub struct RleSink;

impl RleBufWrite for RleSink {
    fn live_cell(&mut self, _n: u64) {}

    fn dead_cell(&mut self, _n: u64) {}

    fn line_break(&mut self, _n: u64) {}

    fn eof(&mut self) {}
}

#[cfg(test)]
mod tests {
    use crate::rle_data::DEAD_CELL_MAGIC;
    use crate::rle_data::EOF_MAGIC;
    use crate::rle_data::LINE_BREAK_MAGIC;
    use crate::rle_data::LIVE_CELL_MAGIC;
    use crate::rle_data::MAGIC_MASK;
    use crate::rle_data::RleBufWrite;
    use crate::rle_data::RleBuffer;

    #[test]
    fn test_insert_empty_file_one() {
        let mut buf = RleBuffer::new();

        buf.live_cell(1);
        buf.eof();

        assert_eq!(
            &buf.data,
            &[LIVE_CELL_MAGIC | MAGIC_MASK, EOF_MAGIC | MAGIC_MASK]
        );
    }

    #[test]
    fn test_insert_empty_file_many() {
        let mut buf = RleBuffer::new();

        buf.live_cell(2);
        buf.eof();

        assert_eq!(
            &buf.data,
            &[2, LIVE_CELL_MAGIC | MAGIC_MASK, EOF_MAGIC | MAGIC_MASK]
        );
    }

    #[test]
    fn test_insert_new_magic_one() {
        let mut buf = RleBuffer::new();

        buf.live_cell(1);
        buf.dead_cell(1);
        buf.eof();

        assert_eq!(
            &buf.data,
            &[
                LIVE_CELL_MAGIC | MAGIC_MASK,
                DEAD_CELL_MAGIC | MAGIC_MASK,
                EOF_MAGIC | MAGIC_MASK
            ]
        );
    }

    #[test]
    fn test_insert_amend_should_merge() {
        let mut buf = RleBuffer::new();

        buf.live_cell(1);
        buf.live_cell(2);
        buf.eof();

        assert_eq!(
            &buf.data,
            &[3, LIVE_CELL_MAGIC | MAGIC_MASK, EOF_MAGIC | MAGIC_MASK]
        );
    }

    #[test]
    fn test_insert_run_length_127() {
        let mut buf = RleBuffer::new();

        buf.live_cell(127);
        buf.eof();

        assert_eq!(
            &buf.data,
            &[127, LIVE_CELL_MAGIC | MAGIC_MASK, EOF_MAGIC | MAGIC_MASK]
        );
    }

    // 128 = 0b1000_0000 -> two 7-bit groups: 0b0000001, 0b0000000
    #[test]
    fn test_insert_run_length_128() {
        let mut buf = RleBuffer::new();

        buf.live_cell(128);
        buf.eof();

        assert_eq!(
            &buf.data,
            &[
                0x01,
                0x00,
                LIVE_CELL_MAGIC | MAGIC_MASK,
                EOF_MAGIC | MAGIC_MASK,
            ]
        );
    }

    #[test]
    fn test_insert_mixed_sequence() {
        let mut buf = RleBuffer::new();

        buf.dead_cell(3);
        buf.live_cell(5);
        buf.line_break(1);
        buf.live_cell(2);
        buf.eof();

        assert_eq!(
            &buf.data,
            &[
                3,
                DEAD_CELL_MAGIC | MAGIC_MASK,
                5,
                LIVE_CELL_MAGIC | MAGIC_MASK,
                LINE_BREAK_MAGIC | MAGIC_MASK,
                2,
                LIVE_CELL_MAGIC | MAGIC_MASK,
                EOF_MAGIC | MAGIC_MASK,
            ]
        );
    }

    #[test]
    fn test_insert_only_eof() {
        let mut buf = RleBuffer::new();

        buf.eof();

        assert_eq!(&buf.data, &[EOF_MAGIC | MAGIC_MASK]);
    }

    /// Run length 128 encodes as 0x80 in the last byte, which has MSB=1.
    /// A decoder scanning byte-by-byte would see 0x80 and interpret it as a
    /// dead cell magic instead of a number byte. This test exhibits the bug:
    /// the number of magic-looking bytes (MSB=1) should be exactly 2 (live cell
    /// + EOF), but the broken encoding produces 3.
    #[test]
    fn test_number_bytes_must_have_msb_zero() {
        let mut buf = RleBuffer::new();

        buf.live_cell(128);
        buf.eof();

        let magic_count = buf.data.iter().filter(|b| *b & MAGIC_MASK != 0).count();

        // Should be exactly 2 magics: live cell + EOF
        assert_eq!(magic_count, 2, "expected 2 magics, got {}", magic_count);
    }

    #[test]
    fn test_insert_line_breaks() {
        let mut buf = RleBuffer::new();

        buf.line_break(5);
        buf.eof();

        assert_eq!(
            &buf.data,
            &[5, LINE_BREAK_MAGIC | MAGIC_MASK, EOF_MAGIC | MAGIC_MASK]
        );
    }
}
