pub enum Action {
    App(AppAction),
    Camera(CameraAction),
    World(WorldAction),
}

pub enum AppAction {
    Quit,
}

pub enum CameraAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ZoomIn,
    ZoomOut,
    ZoomInAt { col: u16, row: u16 },
    ZoomOutAt { col: u16, row: u16 },
    ResetView,
    Resize { cols: u16, rows: u16 },
}

pub enum WorldAction {
    Step,
}
