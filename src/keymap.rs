use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::action::{Action, AppAction, CameraAction};

pub fn resolve(event: Event) -> Option<Action> {
    match event {
        Event::Key(key) => resolve_key(key),
        Event::Mouse(mouse) => resolve_mouse(mouse),
        Event::Resize(cols, rows) => Some(Action::Camera(CameraAction::Resize { cols, rows })),
        _ => None,
    }
}

fn resolve_mouse(mouse: MouseEvent) -> Option<Action> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(Action::Camera(CameraAction::ZoomInAt {
            col: mouse.column,
            row: mouse.row,
        })),
        MouseEventKind::ScrollDown => Some(Action::Camera(CameraAction::ZoomOutAt {
            col: mouse.column,
            row: mouse.row,
        })),
        MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
            Some(Action::Camera(CameraAction::Drag {
                col: mouse.column,
                row: mouse.row,
            }))
        }
        MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
            Some(Action::Camera(CameraAction::DragEnd))
        }
        _ => None,
    }
}

fn resolve_key(key: KeyEvent) -> Option<Action> {
    match key {
        KeyEvent {
            code: KeyCode::Char('q'),
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(Action::App(AppAction::Quit)),

        KeyEvent {
            code: KeyCode::Char('K'),
            modifiers: KeyModifiers::SHIFT,
            ..
        } => Some(Action::Camera(CameraAction::ZoomIn)),

        KeyEvent {
            code: KeyCode::Char('J'),
            modifiers: KeyModifiers::SHIFT,
            ..
        } => Some(Action::Camera(CameraAction::ZoomOut)),

        KeyEvent {
            code: KeyCode::Char('h'),
            ..
        } => Some(Action::Camera(CameraAction::MoveLeft)),

        KeyEvent {
            code: KeyCode::Char('j'),
            ..
        } => Some(Action::Camera(CameraAction::MoveDown)),

        KeyEvent {
            code: KeyCode::Char('k'),
            ..
        } => Some(Action::Camera(CameraAction::MoveUp)),

        KeyEvent {
            code: KeyCode::Char('l'),
            ..
        } => Some(Action::Camera(CameraAction::MoveRight)),

        KeyEvent {
            code: KeyCode::Char('0'),
            ..
        } => Some(Action::Camera(CameraAction::ResetView)),

        KeyEvent {
            code: KeyCode::Char(' '),
            ..
        } => Some(Action::App(AppAction::TogglePlay)),

        KeyEvent {
            code: KeyCode::Char('n'),
            ..
        } => Some(Action::App(AppAction::Step)),

        KeyEvent {
            code: KeyCode::Char(']'),
            ..
        } => Some(Action::App(AppAction::IncreaseK)),

        KeyEvent {
            code: KeyCode::Char('['),
            ..
        } => Some(Action::App(AppAction::DecreaseK)),

        _ => None,
    }
}
