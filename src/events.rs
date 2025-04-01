pub enum Event {
    EngineEvent(EngineEvent),
    AppEvent(AppEvent),
}

pub enum EngineEvent {
    /// Advance the world state by `n`
    Advance(usize),
}

pub enum AppEvent {
    CameraEvent(CameraEvent),

    /// Exit the application
    Exit,
}

pub enum CameraEvent {
    Move,
    Zoom,
}
