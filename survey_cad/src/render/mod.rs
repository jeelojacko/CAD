//! Rendering utilities. Placeholder for drawing CAD entities.

use crate::geometry::Point;

use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin};
use bevy_editor_cam::prelude::*;

const WIDTH: f32 = 640.0;
const HEIGHT: f32 = 480.0;

/// Simple rendering of a point. In real application this would draw to screen.
pub fn render_point(p: Point) {
    let _ = env_logger::builder().is_test(true).try_init();

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (WIDTH, HEIGHT).into(),
                    title: "Survey Point".into(),
                    ..default()
                }),
                ..default()
            }),
            MeshPickingPlugin,
            DefaultEditorCamPlugins,
        ))
        .add_systems(Startup, move |mut commands: Commands| {
            commands.spawn((Camera3d::default(), EditorCam::default()));
            spawn_point(&mut commands, p);
        })
        .run();
}

/// Renders a collection of points. The window will close when requested.
pub fn render_points(points: &[Point]) {
    let points = points.to_vec();
    let _ = env_logger::builder().is_test(true).try_init();

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (WIDTH, HEIGHT).into(),
                    title: "Survey Points".into(),
                    ..default()
                }),
                ..default()
            }),
            MeshPickingPlugin,
            DefaultEditorCamPlugins,
        ))
        .add_systems(Startup, move |mut commands: Commands| {
            commands.spawn((Camera3d::default(), EditorCam::default()));
            for p in &points {
                spawn_point(&mut commands, *p);
            }
        })
        .run();
}

fn spawn_point(commands: &mut Commands, point: Point) {
    commands.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.0, 0.0),
            custom_size: Some(Vec2::splat(4.0)),
            ..default()
        },
        Transform::from_translation(Vec3::new(point.x as f32, point.y as f32, 0.0)),
    ));
}

/// Runs a tiny Bevy application demonstrating ECS usage.
pub fn bevy_ecs_demo() {
    #[derive(Resource)]
    struct Counter(u32);

    fn increment(mut counter: ResMut<Counter>) {
        counter.0 += 1;
        println!("Counter: {}", counter.0);
    }

    let mut app = App::new();
    app.insert_resource(Counter(0));
    app.add_systems(Update, increment);
    app.update();
}
