pub trait RleBufWrite {
    /// Push `n` live cells to the data buffer
    fn live_cell(&mut self, n: u32);

    /// Push `n` dead cells to the data buffer
    fn dead_cell(&mut self, n: u32);

    /// Push `n` line breaks to the data buffer
    fn line_break(&mut self, n: u32);

    /// Push an EOF marker to the data buffer
    fn eof(&mut self);
}

/// A binary representation of an RLE file.
///
/// # Representations
/// All data is u32 aligned. Numbers are pushed as-is.
/// * Live cell:  `0b01`
/// * Dead cell:  `0b01`
/// * Line break: `0b10`
/// * Line break: `0b10`
pub struct RleBuffer {
    data: Vec<u32>,
}

const DEAD_CELL_MAGIC: u32 = 0b00;
const LIVE_CELL_MAGIC: u32 = 0b01;
const LINE_BREAK_MAGIC: u32 = 0b10;
const EOF_MAGIC: u32 = 0b11;

impl RleBufWrite for RleBuffer {
    fn live_cell(&mut self, n: u32) {
        self.insert(LIVE_CELL_MAGIC, n);
    }

    fn dead_cell(&mut self, n: u32) {
        self.insert(DEAD_CELL_MAGIC, n);
    }

    fn line_break(&mut self, n: u32) {
        self.insert(LINE_BREAK_MAGIC, n);
    }

    fn eof(&mut self) {
        if let Some(&EOF_MAGIC) = self.data.last() {
            return;
        }

        self.data.push(EOF_MAGIC)
    }
}

impl RleBuffer {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    fn insert(&mut self, magic: u32, n: u32) {
        match self.data.last() {
            Some(&m) if m == magic => self.insert_amend(magic, n),
            Some(..) | None => self.insert_naive(magic, n),
        }
    }

    #[inline]
    fn insert_naive(&mut self, magic: u32, n: u32) {
        if n > 1 {
            self.data.push(n);
        }

        self.data.push(magic);
    }

    #[inline]
    fn insert_amend(&mut self, magic: u32, n: u32) {
        debug_assert!(!self.data.is_empty());
        debug_assert_eq!(self.data.last().copied(), Some(magic));

        let len = self.data.len();
        if (len == 1) || Self::is_mask(self.data[len - 2]) {
            self.data[len - 1] = n + 1;
            self.data.push(magic);
        } else {
            self.data[len - 2] += n;
        }
    }

    #[inline]
    fn is_mask(entry: u32) -> bool {
        matches!(
            entry,
            LIVE_CELL_MAGIC | DEAD_CELL_MAGIC | LINE_BREAK_MAGIC | EOF_MAGIC
        )
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
    fn live_cell(&mut self, _n: u32) {}

    fn dead_cell(&mut self, _n: u32) {}

    fn line_break(&mut self, _n: u32) {}

    fn eof(&mut self) {}
}

#[cfg(test)]
mod tests {
    use crate::rle_data::DEAD_CELL_MAGIC;
    use crate::rle_data::LIVE_CELL_MAGIC;
    use crate::rle_data::RleBufWrite;
    use crate::rle_data::RleBuffer;

    #[test]
    fn test_insert_empty_file_one() {
        let mut buf = RleBuffer::new();

        buf.live_cell(1);

        assert_eq!(&buf.data, &[LIVE_CELL_MAGIC]);
    }

    #[test]
    fn test_insert_empty_file_many() {
        let mut buf = RleBuffer::new();

        buf.live_cell(2);

        assert_eq!(&buf.data, &[2, LIVE_CELL_MAGIC]);
    }

    #[test]
    fn test_insert_new_magic_one() {
        let mut buf = RleBuffer::new();

        buf.live_cell(1);
        buf.dead_cell(1);

        assert_eq!(&buf.data, &[LIVE_CELL_MAGIC, DEAD_CELL_MAGIC]);
    }

    #[test]
    fn test_insert_amend_should_merge() {
        let mut buf = RleBuffer::new();

        buf.live_cell(1);
        buf.live_cell(2);

        assert_eq!(&buf.data, &[3, LIVE_CELL_MAGIC]);
    }
}
