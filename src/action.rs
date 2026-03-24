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
    ResetView,
    Resize { cols: u16, rows: u16 },
}

pub enum WorldAction {
    Step,
}
