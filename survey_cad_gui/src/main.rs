#![allow(deprecated)]
use bevy::prelude::*;
use clap::Parser;
use survey_cad::{crs::Crs, geometry::Point};

#[derive(Parser)]
struct Args {
    /// EPSG code for the working coordinate system
    #[arg(long, default_value_t = 4326)]
    epsg: u32,
}

#[derive(Resource, Default)]
struct SelectedPoints(Vec<Entity>);

#[derive(Resource, Default)]
struct Dragging(Option<Entity>);

#[derive(Component)]
struct CadPoint;

#[derive(Component)]
struct CadLine {
    start: Entity,
    end: Entity,
}

#[derive(Resource)]
struct WorkingCrs(Crs);

fn main() {
    let args = Args::parse();
    println!("Using EPSG {}", args.epsg);
    App::new()
        .insert_resource(WorkingCrs(Crs::from_epsg(args.epsg)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Survey CAD GUI".into(),
                resolution: (800.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(SelectedPoints::default())
        .insert_resource(Dragging::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (handle_mouse_clicks, drag_point, create_line, update_lines),
        )
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, working: Res<WorkingCrs>) {
    println!("GUI working CRS EPSG: {}", working.0.epsg());
    commands.spawn(Camera2dBundle::default());
    spawn_toolbar(&mut commands, &asset_server);
    // Example content
    spawn_point(&mut commands, Point::new(0.0, 0.0));
}

fn spawn_toolbar(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    commands
        .spawn(NodeBundle {
            node: Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                ..default()
            },
            background_color: BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
            ..default()
        })
        .with_children(|parent| {
            for label in ["File", "Edit", "View"] {
                parent
                    .spawn(ButtonBundle {
                        node: Node {
                            margin: UiRect::all(Val::Px(5.0)),
                            padding: UiRect::new(
                                Val::Px(10.0),
                                Val::Px(10.0),
                                Val::Px(5.0),
                                Val::Px(5.0),
                            ),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                        ..default()
                    })
                    .with_children(|button| {
                        button.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            TextSpan::new(label),
                        ));
                    });
            }
        });
}

fn spawn_point(commands: &mut Commands, p: Point) {
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::srgb(1.0, 0.0, 0.0),
                custom_size: Some(Vec2::splat(5.0)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(p.x as f32, p.y as f32, 0.0)),
            ..default()
        },
        CadPoint,
    ));
}

fn cursor_world_pos(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let (camera, cam_transform) = camera_q.single();
    windows
        .single()
        .cursor_position()
        .and_then(|pos| camera.viewport_to_world_2d(cam_transform, pos).ok())
}

fn handle_mouse_clicks(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    points: Query<(Entity, &Transform), With<CadPoint>>,
    mut selected: ResMut<SelectedPoints>,
    mut dragging: ResMut<Dragging>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        if let Some(pos) = cursor_world_pos(windows, camera_q) {
            let mut hit = None;
            for (e, t) in &points {
                if t.translation.truncate().distance(pos) < 5.0 {
                    hit = Some(e);
                    break;
                }
            }
            if let Some(e) = hit {
                if selected.0.contains(&e) {
                    selected.0.retain(|&x| x != e);
                } else {
                    selected.0.push(e);
                    dragging.0 = Some(e);
                }
            } else {
                spawn_point(&mut commands, Point::new(pos.x as f64, pos.y as f64));
            }
        }
    }

    if buttons.just_released(MouseButton::Left) {
        dragging.0 = None;
    }
}

fn drag_point(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut points: Query<&mut Transform, With<CadPoint>>,
    dragging: Res<Dragging>,
) {
    if let Some(e) = dragging.0 {
        if buttons.pressed(MouseButton::Left) {
            if let Some(pos) = cursor_world_pos(windows, camera_q) {
                if let Ok(mut t) = points.get_mut(e) {
                    t.translation.x = pos.x;
                    t.translation.y = pos.y;
                }
            }
        }
    }
}

fn create_line(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
) {
    if keys.just_pressed(KeyCode::KeyL) && selected.0.len() == 2 {
        let a = points.get(selected.0[0]).unwrap().translation;
        let b = points.get(selected.0[1]).unwrap().translation;
        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::WHITE,
                    custom_size: Some(Vec2::new(a.distance(b), 2.0)),
                    ..default()
                },
                transform: Transform::from_translation((a + b) / 2.0)
                    .with_rotation(Quat::from_rotation_z((b - a).y.atan2((b - a).x))),
                ..default()
            },
            CadLine {
                start: selected.0[0],
                end: selected.0[1],
            },
        ));
    }
}

fn update_lines(
    mut lines: Query<(&CadLine, &mut Transform, &mut Sprite)>,
    points: Query<&Transform, With<CadPoint>>,
) {
    for (line, mut t, mut s) in &mut lines {
        let a = points.get(line.start).unwrap().translation;
        let b = points.get(line.end).unwrap().translation;
        s.custom_size = Some(Vec2::new(a.distance(b), 2.0));
        t.translation = (a + b) / 2.0;
        t.rotation = Quat::from_rotation_z((b - a).y.atan2((b - a).x));
    }
}
