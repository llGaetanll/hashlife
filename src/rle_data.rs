/// A binary representation of an RLE file
///
/// # Representations
/// All data is u32 aligned. Numbers are pushed as-is.
/// * Live cell:  `0b01`
/// * Dead cell:  `0b01`
/// * Line break: `0b10`
/// * Line break: `0b10`
pub struct RleData {
    data: Vec<u32>,
}

const DEAD_CELL_MAGIC: u32 = 0b00;
const LIVE_CELL_MAGIC: u32 = 0b01;
const LINE_BREAK_MAGIC: u32 = 0b10;
const EOF_MAGIC: u32 = 0b11;

impl RleData {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Push `n` live cells to the data buffer
    pub fn live_cell(&mut self, n: u32) {
        debug_assert!(n > 1);

        match self.data.last() {
            Some(&LIVE_CELL_MAGIC) => self.update(LIVE_CELL_MAGIC, n),
            Some(_) | None => {
                self.data.push(n);
                self.data.push(LIVE_CELL_MAGIC);
            }
        }
    }

    /// Push `n` dead cells to the data buffer
    pub fn dead_cell(&mut self, n: u32) {
        debug_assert!(n > 1);

        match self.data.last() {
            Some(&DEAD_CELL_MAGIC) => self.update(DEAD_CELL_MAGIC, n),
            Some(_) | None => {
                self.data.push(n);
                self.data.push(DEAD_CELL_MAGIC);
            }
        }
    }

    /// Push `n` line breaks to the data buffer
    pub fn line_break(&mut self, n: u32) {
        debug_assert!(n > 1);

        match self.data.last() {
            Some(&LINE_BREAK_MAGIC) => self.update(LINE_BREAK_MAGIC, n),
            Some(_) | None => {
                self.data.push(n);
                self.data.push(LINE_BREAK_MAGIC);
            }
        }
    }

    /// Push an EOF marker to the data buffer
    pub fn eof(&mut self) {
        if let Some(&EOF_MAGIC) = self.data.last() {
            return;
        }

        self.data.push(EOF_MAGIC)
    }

    fn update(&mut self, mask: u32, n: u32) {
        let len = self.data.len();
        if len < 2 {
            debug_assert_eq!(len, 1, "Update is only called when buffer is not empty");

            self.data[0] = n;
            self.data.push(mask);
        } else if Self::is_mask(self.data[len - 2]) {
            self.data[len - 1] = n;
            self.data.push(mask);
        } else {
            self.data[len - 2] += n; // NOTE: Possible overflow
        }
    }

    fn is_mask(entry: u32) -> bool {
        matches!(
            entry,
            LIVE_CELL_MAGIC | DEAD_CELL_MAGIC | LINE_BREAK_MAGIC | EOF_MAGIC
        )
    }
}
