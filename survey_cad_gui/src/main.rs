#![allow(deprecated)]
use bevy::prelude::*;
use clap::Parser;
use survey_cad::{crs::Crs, geometry::Point};

use survey_cad::geometry::Point3;

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

#[derive(Resource, Default)]
struct AlignmentData {
    points: Vec<Point>,
}

#[derive(Resource, Default)]
struct SurfaceData {
    vertices: Vec<Point3>,
}

#[derive(Component)]
struct AddAlignmentButton;

#[derive(Component)]
struct AddSurfaceButton;

#[derive(Component)]
struct CorridorButton(CorridorControl);

#[derive(Clone, Copy)]
enum CorridorControl {
    WidthInc,
    WidthDec,
    IntervalInc,
    IntervalDec,
    OffsetInc,
    OffsetDec,
}

#[derive(Resource)]
struct CorridorParams {
    width: f64,
    interval: f64,
    offset_step: f64,
}

impl Default for CorridorParams {
    fn default() -> Self {
        Self {
            width: 5.0,
            interval: 10.0,
            offset_step: 2.5,
        }
    }
}

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
        .insert_resource(AlignmentData::default())
        .insert_resource(SurfaceData::default())
        .insert_resource(CorridorParams::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_mouse_clicks,
                drag_point,
                create_line,
                update_lines,
                handle_add_alignment,
                handle_add_surface,
                handle_corridor_buttons,
            ),
        )
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, working: Res<WorkingCrs>) {
    println!("GUI working CRS EPSG: {}", working.0.epsg());
    commands.spawn(Camera2dBundle::default());
    spawn_toolbar(&mut commands, &asset_server);
    spawn_edit_panel(&mut commands, &asset_server);
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

fn spawn_edit_panel(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    commands
        .spawn(NodeBundle {
            node: Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(30.0),
                width: Val::Px(200.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            background_color: BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                TextLayout::default(),
                TextFont {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 14.0,
                    ..default()
                },
                TextColor::WHITE,
                TextSpan::new("Alignment Editor"),
            ));
            parent.spawn(ButtonBundle::default()).with_children(|b| {
                b.spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    TextSpan::new("Add Selected"),
                ));
            }).insert(AddAlignmentButton);

            parent.spawn((
                TextLayout::default(),
                TextFont {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 14.0,
                    ..default()
                },
                TextColor::WHITE,
                TextSpan::new("Surface Editor"),
            ));
            parent.spawn(ButtonBundle::default()).with_children(|b| {
                b.spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    TextSpan::new("Add Points"),
                ));
            }).insert(AddSurfaceButton);

            parent.spawn((
                TextLayout::default(),
                TextFont {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 14.0,
                    ..default()
                },
                TextColor::WHITE,
                TextSpan::new("Corridor Params"),
            ));
            for (label, ctl) in [
                ("Width -", CorridorControl::WidthDec),
                ("Width +", CorridorControl::WidthInc),
                ("Interval -", CorridorControl::IntervalDec),
                ("Interval +", CorridorControl::IntervalInc),
                ("Offset -", CorridorControl::OffsetDec),
                ("Offset +", CorridorControl::OffsetInc),
            ] {
                parent
                    .spawn(ButtonBundle::default())
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            TextSpan::new(label),
                        ));
                    })
                    .insert(CorridorButton(ctl));
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

fn handle_add_alignment(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<AddAlignmentButton>)>,
    mut data: ResMut<AlignmentData>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        for e in &selected.0 {
            if let Ok(t) = points.get(*e) {
                data.points.push(Point::new(t.translation.x as f64, t.translation.y as f64));
            }
        }
    }
}

fn handle_add_surface(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<AddSurfaceButton>)>,
    mut data: ResMut<SurfaceData>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        for e in &selected.0 {
            if let Ok(t) = points.get(*e) {
                data.vertices.push(Point3::new(t.translation.x as f64, t.translation.y as f64, 0.0));
            }
        }
    }
}

fn handle_corridor_buttons(
    interactions: Query<(&Interaction, &CorridorButton), Changed<Interaction>>,
    mut params: ResMut<CorridorParams>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed {
            match button.0 {
                CorridorControl::WidthInc => params.width += 1.0,
                CorridorControl::WidthDec => params.width -= 1.0,
                CorridorControl::IntervalInc => params.interval += 1.0,
                CorridorControl::IntervalDec => params.interval -= 1.0,
                CorridorControl::OffsetInc => params.offset_step += 0.5,
                CorridorControl::OffsetDec => params.offset_step -= 0.5,
            }
            params.width = params.width.max(0.0);
            params.interval = params.interval.max(0.1);
            params.offset_step = params.offset_step.max(0.1);
            println!(
                "Corridor params -> width: {:.1}, interval: {:.1}, offset: {:.1}",
                params.width, params.interval, params.offset_step
            );
        }
    }
}
