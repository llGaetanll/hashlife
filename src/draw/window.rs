pub struct WindowData {
    pub width: u32,
    pub height: u32,
}

impl WindowData {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}
