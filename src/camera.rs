use crate::cell::Cell;
use crate::cell::LEAF_MASK;
use crate::world::World;

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

pub type ScreenSize = u16;
pub type CellOffset = i16;
pub type WorldOffset = i128;

pub struct Camera {
    /// The cell buffer
    cb: Vec<bool>,

    /// The frame buffer.
    fb: String,

    /// Codepoints. This allows us to construct the framebuffer more easily
    cp: Vec<u32>,

    /// Column width of the framebuffer
    w: ScreenSize,

    /// Column height of the framebuffer
    h: ScreenSize,

    /// `x` offset from origin
    x: WorldOffset,

    /// `y` offset from origin
    y: WorldOffset,

    // World scale expressed in cells as `2^scale`
    scale: u8,
}

// Lateral movement:
// - Scale 0 (1 pixel = 1 cell): moves over 1 px (1 cell)
// - Scale 1 (1 px = 2x2 cell): moves over 1 px (2 cell)
// - Scale 2 (1 px = 4x4 cell): moves over 1 px (4 cell)
// ...
//
// In general: lateral movements *always* move you 1 pixel over, which results in a 2^n movement

impl Camera {
    /// Create a new camera `w` columns wide and `h` columns tall
    pub fn new(w: u16, h: u16) -> Self {
        let (w, h) = (w as usize, h as usize);

        // Cell width and height. Each braille character gives us 2 cells horizontally and 4
        // vertically.
        let (cw, ch) = (w * 2, h * 4);

        // The cell buffer. This keeps track of the individual cells on the screen
        let cb = vec![false; cw * ch];

        // The codepoints buffer. This makes it easier to construct the frame buffer later
        let cp = vec![BRAILLE_EMPTY; w * h];

        // For each braille character, we need 3 bytes:
        //  - The leader byte:     0b11100010
        //  - Continuation byte 1: 0b101000xx
        //  - Continuation byte 2: 0b10xxxxxx
        // For each newline, we need one byte: 0b00001010
        //
        // We need h newlines, this gives us a framebuffer of length `3 * (w * h) + h`
        let mut fb = String::with_capacity(3 * (w * h) + h);

        // Update the frame buffer
        for (i, &c) in cp.iter().enumerate() {
            if i > 0 && i % w == 0 {
                fb.push('\n');
            }

            fb.push(::std::char::from_u32(c).unwrap());
        }
        fb.push('\n');

        Self {
            cb,
            fb,
            cp,
            w: w as ScreenSize,
            h: h as ScreenSize,
            x: 0,
            y: 0,
            scale: 0,
        }
    }

    pub fn width(&self) -> ScreenSize {
        self.w
    }

    pub fn height(&self) -> ScreenSize {
        self.h
    }

    pub fn move_left(&mut self) {
        let dx = 2i128.pow(self.scale as u32);
        self.x += dx;
    }

    pub fn move_right(&mut self) {
        let dx = 2i128.pow(self.scale as u32);
        self.x -= dx;
    }

    pub fn move_up(&mut self) {
        let dy = 2i128.pow(self.scale as u32);
        self.y += dy;
    }

    pub fn move_down(&mut self) {
        let dy = 2i128.pow(self.scale as u32);
        self.y -= dy;
    }

    /// Resize the camera to `w` columns wide, and `h` columns tall
    pub fn resize(&mut self, w: ScreenSize, h: ScreenSize) {
        self.w = w;
        self.h = h;

        let (w, h) = (w as usize, h as usize);

        self.cb.clear();
        self.cb.resize(w * h * 8, false); // We get 8 cells per character using braille

        self.fb.clear();

        self.cp.clear();
        self.cp.resize(w * h, BRAILLE_EMPTY);
    }

    pub fn draw(&mut self, world: &World) {
        let buf = &world.buf;
        let root = world.root;
        let cell = buf[root];
        let n = world.depth as u32 + 3;
        let scale = self.scale as u32;

        // dx and dy here are screen pixel offsets.
        // Since self.x and self.y encode true position, we need to divide them by 2^scale
        let (dx, dy) = (self.x >> self.scale, self.y >> self.scale);

        draw_cell(
            self,
            buf,
            cell,
            dx as CellOffset,
            dy as CellOffset,
            n,
            scale,
        );
    }

    pub fn zoom_in(&mut self) {
        self.scale = self.scale.saturating_sub(1);
    }

    pub fn zoom_out(&mut self) {
        self.scale += 1;
    }

    /// Draw a single pixel of the framebuffer at (`x`, `y`)
    pub fn draw_pixel(&mut self, x: CellOffset, y: CellOffset) {
        let (x, y) = (x as i32, y as i32);
        let (w, h) = (2 * self.w as i32, 4 * self.h as i32);

        if x < 0 || y < 0 || x >= w || y >= h {
            return;
        }

        let i = Self::coords_from(x as ScreenSize, y as ScreenSize, w as usize); // Safe cast

        self.cb[i] = true;
    }

    pub fn draw_outline(&mut self) {
        // Width of cell buffer
        let wid = 2 * self.w as usize;

        for x in 0..self.w {
            let i = Self::coords_from(x, 0, wid);
            let j = Self::coords_from(x, self.h - 1, wid);

            self.cb[i] = true;
            self.cb[j] = true;
        }

        for y in 0..self.h {
            let i = Self::coords_from(0, y, wid);
            let j = Self::coords_from(self.w - 1, y, wid);

            self.cb[i] = true;
            self.cb[j] = true;
        }
    }

    /// Turns on a square grid of pixels in the framebuffer
    pub fn draw_square(&mut self, x: CellOffset, y: CellOffset, s: ScreenSize) {
        self.rect_set(x, y, s, true)
    }

    /// Draw a clear square of sidelength `s` with origin (`x`, `y`) where the origin is taken to
    /// be the top left side of the square.
    pub fn draw_clear_square(&mut self, x: CellOffset, y: CellOffset, s: ScreenSize) {
        self.rect_set(x, y, s, false)
    }

    /// Reset the cell buffer
    pub fn reset(&mut self) {
        self.cb.fill(false);
    }

    pub fn render(&mut self) -> &str {
        // compute new codepoints
        self.cp.fill(BRAILLE_EMPTY);

        let wid = 2 * self.w as usize; // Width of cell buffer
        for (n, &px) in self.cb.iter().enumerate() {
            if px {
                let (x, y) = Self::coords_to(n, wid);
                let i = (y / 4) * self.w + (x / 2);

                let hex = Self::get_hex_value(x, y);

                self.cp[i as usize] += hex;
            }
        }

        // update framebuffer
        self.fb.clear();

        // Update the frame buffer
        let w = self.w as usize;
        for (i, &c) in self.cp.iter().enumerate() {
            if i > 0 && i % w == 0 {
                self.fb.push('\n');
            }

            self.fb.push(::std::char::from_u32(c).unwrap());
        }
        self.fb.push('\n');

        &self.fb
    }

    /// Set a (saturating) rectangle of the cell buffer to a value. Either true, or false.
    fn rect_set(&mut self, x: CellOffset, y: CellOffset, s: ScreenSize, v: bool) {
        let (x, y, s) = (x as i32, y as i32, s as i32);
        let (w, h) = (2 * self.w as i32, 4 * self.h as i32);

        if x + s < 0 || y + s < 0 || x >= w || y >= h {
            return;
        }

        let (x_lo, x_hi) = (0.max(x), w.min(x + s));
        let (y_lo, y_hi) = (0.max(y), h.min(y + s));

        for x in x_lo..x_hi {
            for y in y_lo..y_hi {
                let (x, y) = (x as ScreenSize, y as ScreenSize);

                let i = Self::coords_from(x, y, w as usize);
                self.cb[i] = v;
            }
        }
    }

    fn coords_to(n: usize, width: usize) -> (ScreenSize, ScreenSize) {
        ((n % width) as ScreenSize, (n / width) as ScreenSize)
    }

    fn coords_from(x: ScreenSize, y: ScreenSize, width: usize) -> usize {
        y as usize * width + x as usize
    }

    fn get_hex_value(x: ScreenSize, y: ScreenSize) -> u32 {
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

/// Draws a 4 cell
/// dx and dy are offsets in screen pixels.
fn draw_rule(cam: &mut Camera, rule: u16, dx: CellOffset, dy: CellOffset, scale: u32) {
    match scale {
        // Each rule is 4x4
        // This is the closest zoom possible
        0 => {
            let (mut x, mut y) = (0, 0);
            let mut mask = 1 << 0xF;
            while mask > 0 {
                if rule & mask == mask {
                    cam.draw_pixel(dx + x, dy + y);
                }

                x = (x + 1) % 4;

                if x == 0 {
                    y += 1;
                }

                mask >>= 1;
            }
        }

        // Each rule is 2x2
        1 => {
            let br = rule & 0x33;
            let bl = rule & (0x33 << 2);
            let tr = rule & (0x33 << 8);
            let tl = rule & (0x33 << 10);

            if tl != 0 {
                cam.draw_pixel(dx, dy);
            }

            if tr != 0 {
                cam.draw_pixel(dx + 1, dy);
            }

            if bl != 0 {
                cam.draw_pixel(dx, dy + 1);
            }

            if br != 0 {
                cam.draw_pixel(dx + 1, dy + 1);
            }
        }

        // Each rule is 1x1
        2 => {
            if rule != 0 {
                cam.draw_pixel(dx, dy);
            }
        }

        // Too small to draw
        _ => {}
    }
}

fn draw_leaf(cam: &mut Camera, cell: Cell, dx: CellOffset, dy: CellOffset, scale: u32) {
    assert!(cell.is_leaf());

    match scale {
        // Each leaf is 8x8. At this scale, each screen pixel is exactly 1 cell
        0 => {
            draw_rule(cam, (cell.nw & !LEAF_MASK) as u16, dx, dy, scale);
            draw_rule(cam, cell.ne as u16, dx + 4, dy, scale);
            draw_rule(cam, cell.sw as u16, dx, dy + 4, scale);
            draw_rule(cam, cell.se as u16, dx + 4, dy + 4, scale);
        }

        // Each leaf is 4x4. At this scale, each screen pixel is a 2x2 array of cells
        1 => {
            draw_rule(cam, (cell.nw & !LEAF_MASK) as u16, dx, dy, scale);
            draw_rule(cam, cell.ne as u16, dx + 2, dy, scale);
            draw_rule(cam, cell.sw as u16, dx, dy + 2, scale);
            draw_rule(cam, cell.se as u16, dx + 2, dy + 2, scale);
        }

        // Each leaf is 2x2. At this scale, each screen pixel is a 4x4 array of cells
        2 => {
            draw_rule(cam, (cell.nw & !LEAF_MASK) as u16, dx, dy, scale);
            draw_rule(cam, cell.ne as u16, dx + 1, dy, scale);
            draw_rule(cam, cell.sw as u16, dx, dy + 1, scale);
            draw_rule(cam, cell.se as u16, dx + 1, dy + 1, scale);
        }

        // Each leaf is 1x1
        3 => {
            if !cell.is_void() {
                cam.draw_pixel(dx, dy);
            }
        }

        // Too small to see
        _ => {}
    }
}

/// Draw a `2^n` cell. It's important to note here that n >= 3. n = 3 is a leaf
fn draw_cell(
    cam: &mut Camera,
    buf: &[Cell],
    cell: Cell,
    dx: CellOffset,
    dy: CellOffset,
    n: u32,
    scale: u32,
) {
    // Too small to draw
    if scale > n {
        return;
    }

    // The square width of a node
    let sw = 2u16.saturating_pow(n - scale);

    // Empty 2^n cell
    if cell.is_void() {
        cam.draw_clear_square(dx, dy, sw);

    // Single pixel cell
    } else if sw == 1 {
        cam.draw_pixel(dx, dy);

    // Leaf cell
    } else if n == 3 {
        draw_leaf(cam, cell, dx, dy, scale);

    // Non-leaf cell
    } else {
        // As we recurse, the square width of nodes is halved
        let sw = (sw >> 1) as CellOffset; // TODO: Not sure about this cast

        draw_cell(cam, buf, buf[cell.ne], dx, dy, n - 1, scale);
        draw_cell(cam, buf, buf[cell.nw], dx + sw, dy, n - 1, scale);
        draw_cell(cam, buf, buf[cell.se], dx, dy + sw, n - 1, scale);
        draw_cell(cam, buf, buf[cell.sw], dx + sw, dy + sw, n - 1, scale);
    }
}
