use sdl2::pixels::Color;
use sdl2::rect::Point;
use sdl2::render::WindowCanvas;

use crate::draw::window::WindowData;
use crate::ext::sdl::ColorInterpolationExt;

pub struct GridData {
    /// multiplier on `base_cell_size`
    zoom_level: u32,

    /// When zooming in or out, this is the multiplier
    zoom_mult: f64,

    /// The most you can zoom in on a cell
    base_cell_size: u32,

    // precomputed for interpolation when computing gridline colors
    max_zoom_level: u32,

    // `window_offsets[i]` represents the window offset for `zoom_level` `i`. The reason that
    // offsets are stored this way is to avoid loss of precision that comes with floating point
    // numbers. This set of coordinates allows us completely reconstruct the position of the camera
    // on the grid.
    window_offsets: Vec<(i32, i32)>,

    show_gridlines: bool,

    // base color of the gridlines
    gridline_color: Color,
    background_color: Color,
}

impl GridData {
    pub fn new() -> Self {
        // these are probably fine constants
        let zoom_mult = 1.3;
        let base_cell_size = 50;

        Self {
            zoom_level: 0,
            zoom_mult,
            base_cell_size,
            max_zoom_level: Self::get_max_zoom_level(base_cell_size, zoom_mult),

            window_offsets: vec![(0, 0)],

            show_gridlines: true,

            gridline_color: Color::RGB(60, 60, 60),
            background_color: Color::WHITE,
        }
    }

    pub fn toggle_gridlines(&mut self) {
        self.show_gridlines = !self.show_gridlines;
    }

    pub fn shift_x(&mut self, dx: i32) {
        Self::offset(self, dx, 0);
    }

    pub fn shift_y(&mut self, dy: i32) {
        Self::offset(self, 0, dy);
    }

    pub fn offset(&mut self, dx: i32, dy: i32) {
        self.window_offsets[self.zoom_level as usize].0 += dx;
        self.window_offsets[self.zoom_level as usize].1 += dy;
    }

    pub fn reset_zoom(&mut self) {
        self.zoom_level = 0;
    }

    pub fn zoom_in(&mut self) {
        if self.zoom_level == 0 {
            return;
        }

        self.zoom_level -= 1;

        // update x and y offsets
        self.offset(0, 0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom_level += 1;

        // only add to window_offsets if needed
        if self.zoom_level as usize == self.window_offsets.len() {
            self.window_offsets.push((0, 0));
        }

        // update x and y offsets
        self.offset(0, 0);
    }

    pub fn line_spacing(&self) -> Option<u32> {
        let res = (self.base_cell_size as f64) / self.zoom_mult.powf(self.zoom_level as f64);

        if res < 1f64 {
            None
        } else {
            Some(res as u32)
        }
    }

    pub fn draw(&self, canvas: &mut WindowCanvas, window: &WindowData) -> Result<(), String> {
        let spacing = self.line_spacing();

        // set background color
        canvas.set_draw_color(self.background_color);
        canvas.clear();

        // draw grid lines
        if let Some(spacing) = spacing {
            if !self.show_gridlines {
                return Ok(());
            }

            let gridline_color = self.get_gridline_color();
            canvas.set_draw_color(gridline_color);

            let (dx, dy) = self.window_offsets[self.zoom_level as usize];
            let (dx, dy) = (
                (dx + dx.abs() * spacing as i32) % spacing as i32,
                (dy + dy.abs() * spacing as i32) % spacing as i32,
            );

            // vertical lines
            let num_lines = window.width.div_ceil(spacing);
            for i in 0..num_lines {
                let delta = dx + (i * spacing) as i32;

                canvas.draw_line(
                    Point::new(delta, 0),
                    Point::new(delta, window.height as i32),
                )?;
            }

            // horizontal lines
            let num_lines = window.height.div_ceil(spacing);
            for i in 0..num_lines {
                let delta = dy + (i * spacing) as i32;

                canvas.draw_line(Point::new(0, delta), Point::new(window.width as i32, delta))?;
            }
        }

        Ok(())
    }

    // As we zoom out, the grid lines fade more and more into the background
    pub fn get_gridline_color(&self) -> Color {
        let p = (self.max_zoom_level - self.zoom_level) as f64 / self.max_zoom_level as f64;
        self.gridline_color.lerp(&self.background_color, p)
    }

    /// Computes the maximum number `k` such that
    ///
    ///    D / Z^k >= 1
    ///
    /// Where:
    ///   - `D` is the base cell size
    ///   - `Z` is the zoom multiplier
    pub fn get_max_zoom_level(base_cell_size: u32, zoom_mult: f64) -> u32 {
        let cell_size = base_cell_size as f64;

        cell_size.log(zoom_mult).floor() as u32
    }
}
