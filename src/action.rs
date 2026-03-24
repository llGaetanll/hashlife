pub enum Action {
    App(AppAction),
    Camera(CameraAction),
}

pub enum AppAction {
    Quit,
    TogglePlay,
    Step,
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
    Drag { col: u16, row: u16 },
    DragEnd,
    ResetView,
    Resize { cols: u16, rows: u16 },
}
