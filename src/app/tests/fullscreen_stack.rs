//! The Fullscreen Stack Lab (`[labs] fullscreen_stack`): tiles that join a
//! Fullscreen Layout workspace stack instead of exiting the mode, and
//! directional focus wraps around the stack. The default (Lab off) behavior is
//! covered in `fullscreen.rs`.

use super::*;

fn lab() -> Config {
    Config::parse("version = 1\n[labs]\nfullscreen_stack = true\n").unwrap()
}

#[test]
fn lab_directional_focus_wraps_around_a_fullscreen_stack() {
    let first = window(10);
    let second = window(20);
    let third = window(30);
    let neighbor = window(40);
    let outputs = outputs();
    let mut backend = FakeBackend {
        focused: Some(second.clone()),
        ..FakeBackend::default()
    };
    for window in [&first, &second, &third] {
        backend.frames.insert(window.clone(), frame(50));
    }
    backend.frames.insert(neighbor.clone(), frame(1200));
    let mut runtime = runtime_with_config(backend, lab());
    for window in [&first, &second, &third] {
        bind(&mut runtime, WorkspaceId::ONE, window);
    }
    bind(&mut runtime, WorkspaceId::SIX, &neighbor);
    let cancelled = AtomicBool::new(false);
    runtime
        .toggle_fullscreen_layout(outputs.clone(), &cancelled)
        .unwrap();
    let focus = |runtime: &mut WorkspaceRuntime<FakeBackend>, direction| {
        runtime
            .focus_relative(direction, outputs.clone(), &cancelled)
            .unwrap();
        runtime.backend.focused.clone().unwrap().window_id
    };

    assert_eq!(focus(&mut runtime, Direction::Left), 10);
    // The start of the stack wraps to the end instead of stopping.
    assert_eq!(focus(&mut runtime, Direction::Up), 30);
    assert_eq!(focus(&mut runtime, Direction::Up), 20);
    // The end of the stack wraps to the start instead of leaving the display,
    // so holding one direction cycles every tile.
    assert_eq!(focus(&mut runtime, Direction::Right), 30);
    assert_eq!(focus(&mut runtime, Direction::Right), 10);
    assert_eq!(focus(&mut runtime, Direction::Down), 20);
    assert_eq!(focus(&mut runtime, Direction::Down), 30);
    assert_eq!(focus(&mut runtime, Direction::Down), 10);
    assert!(runtime.workspaces.uses_fullscreen_layout(WorkspaceId::ONE));
}

#[test]
fn lab_directional_focus_from_a_lone_fullscreen_tile_crosses_displays() {
    let only = window(10);
    let neighbor = window(40);
    let outputs = outputs();
    let mut backend = FakeBackend {
        focused: Some(only.clone()),
        ..FakeBackend::default()
    };
    backend.frames.insert(only.clone(), frame(50));
    backend.frames.insert(neighbor.clone(), frame(1200));
    let mut runtime = runtime_with_config(backend, lab());
    bind(&mut runtime, WorkspaceId::ONE, &only);
    bind(&mut runtime, WorkspaceId::SIX, &neighbor);
    let cancelled = AtomicBool::new(false);
    runtime
        .toggle_fullscreen_layout(outputs.clone(), &cancelled)
        .unwrap();

    runtime
        .focus_relative(Direction::Right, outputs, &cancelled)
        .unwrap();

    assert_eq!(runtime.backend.focused.clone().unwrap().window_id, 40);
}

#[test]
fn lab_opening_a_new_tiled_window_joins_the_fullscreen_stack() {
    let first = window(10);
    let second = window(20);
    let outputs = outputs();
    let cancelled = AtomicBool::new(false);
    let mut backend = FakeBackend {
        focused: Some(first.clone()),
        observation: Some(observation(&first, frame(50))),
        ..FakeBackend::default()
    };
    backend.frames.insert(first.clone(), frame(50));
    let mut runtime = runtime_with_config(backend, lab());
    runtime.cycle(outputs.clone(), &cancelled).unwrap();
    runtime
        .toggle_fullscreen_layout(outputs.clone(), &cancelled)
        .unwrap();

    runtime.backend.frames.insert(second.clone(), frame(400));
    runtime.backend.focused = Some(second.clone());
    runtime.backend.observation = Some(observation_for_windows(
        "com.example.Test",
        &second,
        &[
            (first.clone(), outputs.main().frame),
            (second.clone(), frame(400)),
        ],
    ));
    let status = runtime.cycle(outputs.clone(), &cancelled).unwrap();

    assert!(runtime.workspaces.uses_fullscreen_layout(WorkspaceId::ONE));
    assert!(matches!(status.layouts[0].mode, UiHudLayout::Fullscreen));
    assert_eq!(runtime.backend.frames[&first], outputs.main().frame);
    assert_eq!(runtime.backend.frames[&second], outputs.main().frame);

    // Leaving the mode still splits every tile that joined the stack.
    runtime
        .toggle_fullscreen_layout(outputs.clone(), &cancelled)
        .unwrap();
    assert!(!runtime.workspaces.uses_fullscreen_layout(WorkspaceId::ONE));
    assert_eq!(runtime.backend.frames[&first].width, 500);
    assert_eq!(runtime.backend.frames[&second].x, 500);
    assert_eq!(runtime.backend.frames[&second].width, 500);
}

/// Moves a focused tile from the secondary display's workspace into a visible
/// Fullscreen Layout workspace on the main display.
fn move_into_fullscreen(config: Config) -> WorkspaceRuntime<FakeBackend> {
    let resident = window(10);
    let mover = window(40);
    let mut backend = FakeBackend {
        focused: Some(resident.clone()),
        ..FakeBackend::default()
    };
    backend.frames.insert(resident.clone(), frame(50));
    backend.frames.insert(mover.clone(), frame(1200));
    let mut runtime = runtime_with_config(backend, config);
    bind(&mut runtime, WorkspaceId::ONE, &resident);
    bind(&mut runtime, WorkspaceId::SIX, &mover);
    let cancelled = AtomicBool::new(false);
    runtime
        .toggle_fullscreen_layout(outputs(), &cancelled)
        .unwrap();
    assert!(runtime.workspaces.uses_fullscreen_layout(WorkspaceId::ONE));

    runtime.backend.focused = Some(mover.clone());
    runtime
        .move_focused(WorkspaceId::ONE, outputs(), &cancelled)
        .unwrap();
    assert_eq!(
        runtime.workspaces.workspace_of(&mover),
        Some(WorkspaceId::ONE)
    );
    runtime
}

#[test]
fn lab_moving_a_tile_in_joins_the_fullscreen_stack() {
    let runtime = move_into_fullscreen(lab());
    assert!(runtime.workspaces.uses_fullscreen_layout(WorkspaceId::ONE));
}

#[test]
fn without_the_lab_moving_a_tile_in_exits_fullscreen_layout() {
    let runtime = move_into_fullscreen(Config::default());
    assert!(!runtime.workspaces.uses_fullscreen_layout(WorkspaceId::ONE));
}

/// #153 path: a parked tile its frontmost app brings back onto a visible
/// Fullscreen Layout workspace joins the stack with the Lab on.
#[test]
fn lab_reappearing_parked_tile_joins_the_fullscreen_stack() {
    let first = window(10);
    let second = window(20);
    let returning = window(30);
    let visible = [(first.clone(), frame(40)), (second.clone(), frame(500))];
    let mut backend = FakeBackend {
        focused: Some(first.clone()),
        frontmost: Some(first.app.clone()),
        observation: Some(observation_for_windows(
            "com.example.Test",
            &first,
            &visible,
        )),
        frames: visible.into_iter().collect(),
        ..FakeBackend::default()
    };
    backend
        .frames
        .insert(returning.clone(), main_parked_frame(frame(100)));
    let mut rt = runtime_with_parked(backend, [(returning.clone(), frame(100))]);
    rt.replace_config(lab());
    bind(&mut rt, WorkspaceId::ONE, &first);
    bind(&mut rt, WorkspaceId::ONE, &second);
    bind(&mut rt, WorkspaceId::new(2).unwrap(), &returning);
    let cancelled = AtomicBool::new(false);
    rt.cycle(outputs(), &cancelled).unwrap();
    rt.toggle_fullscreen_layout(outputs(), &cancelled).unwrap();

    rt.backend.frames.insert(returning.clone(), frame(100));
    rt.backend.observation = Some(observation_for_windows(
        "com.example.Test",
        &first,
        &[
            (first.clone(), frame(40)),
            (second.clone(), frame(500)),
            (returning.clone(), frame(100)),
        ],
    ));
    rt.cycle(outputs(), &cancelled).unwrap();

    assert_eq!(
        rt.workspaces.workspace_of(&returning),
        Some(WorkspaceId::ONE)
    );
    assert!(rt.workspaces.uses_fullscreen_layout(WorkspaceId::ONE));
    let frames = [&first, &second, &returning].map(|id| rt.backend.frames[id]);
    assert_eq!(frames[0], frames[1]);
    assert_eq!(frames[0], frames[2]);
}
