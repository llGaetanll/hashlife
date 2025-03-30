/// Hex values of braille dots
///  
///      1   8
///      2  10
///      4  20
///     40  80
///
/// Where the base blank pattern is codepoint `0x2800` (or U+2800)
///
/// To get other configurations, just add the numbers above.
const BRAILLE_EMPTY: u32 = 0x2800;

pub struct Camera {
    /// The cell buffer
    cb: Vec<bool>,

    /// The frame buffer.
    fb: String,

    /// Codepoints. This allows us to construct the framebuffer more easily
    cp: Vec<u32>,

    /// Width of the framebuffer
    w: usize,

    /// Height of the framebuffer
    h: usize,

    /// `x` offset from origin
    x: i32,

    /// `y` offset from origin
    y: i32,
}

impl Camera {
    pub fn new(w: usize, h: usize) -> Self {
        let cb = vec![false; w * h];

        // For each braille character, we need 3 bytes:
        //  - The leader byte:     0b11100010
        //  - Continuation byte 1: 0b101000xx
        //  - Continuation byte 2: 0b10xxxxxx
        // For each newline, we need one byte: 0b00001010
        //
        // Let `w` and `h` refer to width and height of the cell buffer. Then `bw = ceil(w / 2)`
        // and `bh = ceil(h / 4)` are the width and height of braille characters of our framebuffer
        // (that is, not accounting for the trailing newlines expected at the end of each line).

        let (bw, bh) = (w.div_ceil(2), h.div_ceil(4));
        let cp = vec![BRAILLE_EMPTY; bw * bh];

        // Each braille character is 3 bytes, and newlines one byte. Since we need `bh` newlines,
        // this gives us a framebuffer of length `3 * (bw * bh) + bh`.

        let mut fb = String::with_capacity(3 * (bw * bh) + bh);

        // Update the frame buffer
        for (i, &c) in cp.iter().enumerate() {
            if i > 0 && i % bw == 0 {
                fb.push('\n');
            }

            fb.push(::std::char::from_u32(c).unwrap());
        }
        fb.push('\n');

        Self {
            cb,
            fb,
            cp,
            w,
            h,
            x: 0,
            y: 0,
        }
    }

    pub fn width(&self) -> usize {
        self.w
    }

    pub fn height(&self) -> usize {
        self.h
    }

    pub fn offset_x(&mut self, offset: i32) {
        self.x += offset;
    }

    pub fn offset_y(&mut self, offset: i32) {
        self.y += offset;
    }

    /// Turns on a single pixel of the framebuffer
    pub fn draw_pixel(&mut self, x: usize, y: usize) {
        assert!(x < self.w, "x is out of bounds");
        assert!(y < self.h, "y is out of bounds");

        let i = self.xy_from(x, y);

        self.cb[i] = true;
    }

    /// Turns on a square grid of pixels in the framebuffer
    pub fn draw_square(&mut self, x: usize, y: usize, s: usize) {
        assert!(x < self.w - s, "x is out of bounds");
        assert!(y < self.h - s, "y is out of bounds");

        for dx in 0..s {
            for dy in 0..s {
                let (x, y) = (x + dx, y + dy);

                let i = self.xy_from(x, y);
                self.cb[i] = true
            }
        }
    }

    /// Reset the cell buffer
    pub fn reset(&mut self) {
        self.cb.fill(false);
    }

    /// Fundamentally, we have a framebuffer of every pixel on our screen, and we ask ourselves "Is
    /// this pixel on or off?". This will be the technique used for drawing the tree later on
    pub fn render(&mut self) -> &str {
        let bw = self.w.div_ceil(2);

        // compute new codepoints
        self.cp.fill(BRAILLE_EMPTY);

        for (n, &px) in self.cb.iter().enumerate() {
            let (x, y) = self.xy_to(n);
            let hex = Self::get_hex_value(x, y);

            if px {
                self.cp[(y / 4) * bw + (x / 2)] += hex;
            }
        }

        // update framebuffer
        self.fb.clear();

        // Update the frame buffer
        for (i, &c) in self.cp.iter().enumerate() {
            if i > 0 && i % bw == 0 {
                self.fb.push('\n');
            }

            self.fb.push(::std::char::from_u32(c).unwrap());
        }
        self.fb.push('\n');

        &self.fb
    }

    fn xy_to(&self, n: usize) -> (usize, usize) {
        (n % self.w, n / self.w)
    }

    fn xy_from(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    fn get_hex_value(x: usize, y: usize) -> u32 {
        match (x % 2, y % 4) {
            (0, 0) => 0x1,
            (1, 0) => 0x8,
            (0, 1) => 0x2,
            (1, 1) => 0x10,
            (0, 2) => 0x4,
            (1, 2) => 0x20,
            (0, 3) => 0x40,
            (1, 3) => 0x80,
            _ => unreachable!(),
        }
    }
}
