pub struct WindowContext {
    /// Framebuffer. This is an array of booleans where fb[x, y] represents whether to draw pixel
    /// (x, y) of the screen as dead or alive.
    ///
    /// TODO: use u64s in the future for more efficient packings
    pub fb: Vec<bool>,

    pub x: i32,
    pub y: i32,

    /// Width of the screen in pixels
    pub width: u32,

    /// Height of the screen in pixels
    pub height: u32,

    // lower means more zoomed in, higher means more zoomed out
    pub zoom_level: i32,
}

impl WindowContext {
    pub fn new(width: u32, height: u32) -> Self {
        let fb_size = (width as usize) * (height as usize);
        Self {
            fb: vec![false; fb_size],
            x: 0, 
            y: 0,
            width,
            height,
            zoom_level: 0
        }
    }
}

