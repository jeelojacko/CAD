#![allow(deprecated)]
use bevy::prelude::*;
use clap::Parser;
use std::collections::HashMap;
use survey_cad::geometry::Point3;
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

#[derive(Resource, Default)]
struct AlignmentData {
    points: Vec<Entity>,
}

#[derive(Resource, Default)]
struct SurfaceData {
    vertices: Vec<Point3>,
    breaklines: Vec<(usize, usize)>,
    holes: Vec<Vec<usize>>,
    point_map: HashMap<Entity, usize>,
}

#[derive(Resource, Default)]
struct SurfaceTin(Option<survey_cad::dtm::Tin>);

#[derive(Component)]
struct SurfaceMesh;

#[derive(Component)]
struct BuildSurfaceButton;

#[derive(Component)]
struct ShowProfileButton;

#[derive(Component)]
struct ShowSectionsButton;

#[derive(Component)]
struct ProfileLine;

#[derive(Component)]
struct SectionLine;

#[derive(Component)]
struct AddAlignmentButton;

#[derive(Component)]
struct AlignmentLine;

#[derive(Component)]
struct AddSurfaceButton;

#[derive(Component)]
struct AddBreaklineButton;

#[derive(Component)]
struct AddHoleButton;

#[derive(Component)]
struct AddParcelButton;

#[derive(Component)]
struct GradeButton;

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

#[derive(Resource, Default)]
struct ProfileVisible(bool);

#[derive(Resource, Default)]
struct SectionsVisible(bool);

#[derive(Resource, Default)]
struct ParcelData {
    parcels: Vec<survey_cad::parcel::Parcel>,
    text: Option<Entity>,
}

#[derive(Resource, Default)]
struct GradeInfo {
    text: Option<Entity>,
}

#[derive(Resource, Default)]
struct SectionView {
    sections: Vec<survey_cad::corridor::CrossSection>,
    current: usize,
    entities: Vec<Entity>,
    label: Option<Entity>,
}

#[derive(Component)]
struct PrevSectionButton;

#[derive(Component)]
struct NextSectionButton;

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
        .insert_resource(SurfaceTin::default())
        .insert_resource(CorridorParams::default())
        .insert_resource(ProfileVisible::default())
        .insert_resource(SectionsVisible::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_mouse_clicks,
                drag_point,
                create_line,
                update_lines,
                update_alignment_lines,
                handle_add_alignment,
                handle_add_surface,
                handle_add_breakline,
                handle_add_hole,
                handle_add_parcel,
                handle_corridor_buttons,
                handle_build_surface,
                handle_grade_button,
                handle_show_profile,
                handle_show_sections,
                handle_section_nav,
                update_profile_lines,
                update_section_lines,
            ),
        )
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, working: Res<WorkingCrs>) {
    println!("GUI working CRS EPSG: {}", working.0.epsg());
    commands.spawn(Camera2dBundle::default());
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, -50.0, 50.0).looking_at(Vec3::ZERO, Vec3::Z),
        ..default()
    });
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: false,
            ..default()
        },
        ..default()
    });
    spawn_toolbar(&mut commands, &asset_server);
    let (parcel_text, grade_text) = spawn_edit_panel(&mut commands, &asset_server);
    let section_label = spawn_sections_panel(&mut commands, &asset_server);
    commands.insert_resource(ParcelData { parcels: Vec::new(), text: Some(parcel_text) });
    commands.insert_resource(GradeInfo { text: Some(grade_text) });
    commands.insert_resource(SectionView { label: Some(section_label), ..Default::default() });
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

fn spawn_edit_panel(commands: &mut Commands, asset_server: &Res<AssetServer>) -> (Entity, Entity) {
    let mut parcel_text = Entity::from_raw(0);
    let mut grade_text = Entity::from_raw(0);
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
                        TextSpan::new("Add Selected"),
                    ));
                })
                .insert(AddAlignmentButton);

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
                        TextSpan::new("Add Points"),
                    ));
                })
                .insert(AddSurfaceButton);

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
                        TextSpan::new("Add Breakline"),
                    ));
                })
                .insert(AddBreaklineButton);

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
                        TextSpan::new("Add Hole"),
                    ));
                })
                .insert(AddHoleButton);

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
                        TextSpan::new("Add Parcel"),
                    ));
                })
                .insert(AddParcelButton);

            parcel_text = parent
                .spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    TextSpan::new("Parcel Area: 0"),
                ))
                .id();

            grade_text = parent
                .spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    TextSpan::new("Grade Result:"),
                ))
                .id();

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
                        TextSpan::new("Build Surface"),
                    ));
                })
                .insert(BuildSurfaceButton);

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
                        TextSpan::new("Grade Slope"),
                    ));
                })
                .insert(GradeButton);

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
                        TextSpan::new("Show Profile"),
                    ));
                })
                .insert(ShowProfileButton);

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
                        TextSpan::new("Show Sections"),
                    ));
                })
                .insert(ShowSectionsButton);
        });
    (parcel_text, grade_text)
}

fn spawn_sections_panel(commands: &mut Commands, asset_server: &Res<AssetServer>) -> Entity {
    let mut label = Entity::from_raw(0);
    commands
        .spawn(NodeBundle {
            node: Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(80.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            background_color: BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
            ..default()
        })
        .with_children(|parent| {
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
                        TextSpan::new("Prev"),
                    ));
                })
                .insert(PrevSectionButton);
            label = parent
                .spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    TextSpan::new("Station: 0.0"),
                ))
                .id();
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
                        TextSpan::new("Next"),
                    ));
                })
                .insert(NextSectionButton);
        });
    label
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
    interaction: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<AddAlignmentButton>),
    >,
    mut data: ResMut<AlignmentData>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        for e in &selected.0 {
            if points.get(*e).is_ok() && !data.points.contains(e) {
                data.points.push(*e);
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
                let idx = data.vertices.len();
                data.vertices.push(Point3::new(
                    t.translation.x as f64,
                    t.translation.y as f64,
                    0.0,
                ));
                data.point_map.insert(*e, idx);
            }
        }
    }
}

fn get_vertex_index(data: &mut SurfaceData, e: Entity, t: &Transform) -> usize {
    if let Some(&idx) = data.point_map.get(&e) {
        idx
    } else {
        let idx = data.vertices.len();
        data.vertices.push(Point3::new(
            t.translation.x as f64,
            t.translation.y as f64,
            0.0,
        ));
        data.point_map.insert(e, idx);
        idx
    }
}

fn handle_add_breakline(
    interaction: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<AddBreaklineButton>),
    >,
    mut data: ResMut<SurfaceData>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if selected.0.len() >= 2 {
            let a = selected.0[0];
            let b = selected.0[1];
            if let (Ok(t1), Ok(t2)) = (points.get(a), points.get(b)) {
                let i1 = get_vertex_index(&mut data, a, t1);
                let i2 = get_vertex_index(&mut data, b, t2);
                data.breaklines.push((i1, i2));
                println!("Added breakline between {} and {}", i1, i2);
            }
        }
    }
}

fn handle_add_hole(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<AddHoleButton>)>,
    mut data: ResMut<SurfaceData>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if selected.0.len() >= 3 {
            let mut hole = Vec::new();
            for e in &selected.0 {
                if let Ok(t) = points.get(*e) {
                    let idx = get_vertex_index(&mut data, *e, t);
                    hole.push(idx);
                }
            }
            data.holes.push(hole);
            println!(
                "Added hole with {} vertices",
                data.holes.last().unwrap().len()
            );
        }
    }
}

fn handle_add_parcel(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<AddParcelButton>)>,
    mut parcels: ResMut<ParcelData>,
    selected: Res<SelectedPoints>,
    points: Query<&Transform, With<CadPoint>>,
    mut texts: Query<&mut TextSpan>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if selected.0.len() >= 3 {
            let mut pts = Vec::new();
            for e in &selected.0 {
                if let Ok(t) = points.get(*e) {
                    pts.push(Point::new(t.translation.x as f64, t.translation.y as f64));
                }
            }
            let parcel = survey_cad::parcel::Parcel::new(pts);
            let area = parcel.area();
            parcels.parcels.push(parcel);
            if let Some(id) = parcels.text {
                if let Ok(mut span) = texts.get_mut(id) {
                    span.0 = format!("Parcel Area: {:.2}", area);
                }
            }
            println!("Parcel area: {:.2}", area);
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

fn build_tin(data: &SurfaceData) -> survey_cad::dtm::Tin {
    survey_cad::dtm::Tin::from_points_constrained_with_holes(
        data.vertices.clone(),
        Some(&data.breaklines),
        None,
        &data.holes,
    )
}

fn handle_build_surface(
    interaction: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<BuildSurfaceButton>),
    >,
    data: Res<SurfaceData>,
    mut tin_res: ResMut<SurfaceTin>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<SurfaceMesh>>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        for e in &existing {
            commands.entity(e).despawn_recursive();
        }
        let tin = build_tin(&data);
        let mesh = build_surface_mesh(&tin);
        let handle = meshes.add(mesh);
        let mat = materials.add(StandardMaterial {
            base_color: Color::GREEN,
            ..default()
        });
        commands
            .spawn(PbrBundle {
                mesh: handle,
                material: mat,
                ..default()
            })
            .insert(SurfaceMesh);
        tin_res.0 = Some(tin);
    }
}

fn build_surface_mesh(tin: &survey_cad::dtm::Tin) -> Mesh {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    let positions: Vec<[f32; 3]> = tin
        .vertices
        .iter()
        .map(|p| [p.x as f32, p.y as f32, p.z as f32])
        .collect();
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let uvs = vec![[0.0, 0.0]; positions.len()];
    let indices: Vec<u32> = tin
        .triangles
        .iter()
        .flat_map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.set_indices(Some(Indices::U32(indices)));
    mesh
}

fn handle_show_profile(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<ShowProfileButton>)>,
    mut visible: ResMut<ProfileVisible>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        visible.0 = !visible.0;
    }
}

fn handle_show_sections(
    interaction: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<ShowSectionsButton>),
    >,
    mut visible: ResMut<SectionsVisible>,
    tin_res: Res<SurfaceTin>,
    data: Res<AlignmentData>,
    params: Res<CorridorParams>,
    points: Query<&Transform, With<CadPoint>>,
    mut view: ResMut<SectionView>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        visible.0 = !visible.0;
        view.entities.clear();
        if visible.0 {
            view.sections.clear();
            view.current = 0;
            if let (Some(tin), true) = (tin_res.0.as_ref(), data.points.len() > 1) {
                use survey_cad::alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
                use survey_cad::corridor::extract_cross_sections;
                let mut pts = Vec::new();
                let mut v_pairs = Vec::new();
                for (i, e) in data.points.iter().enumerate() {
                    if let Ok(t) = points.get(*e) {
                        pts.push(Point::new(t.translation.x as f64, t.translation.y as f64));
                        v_pairs.push((i as f64, t.translation.y as f64));
                    }
                }
                let hal = HorizontalAlignment::new(pts);
                let val = VerticalAlignment::new(v_pairs);
                let align = Alignment::new(hal, val);
                view.sections = extract_cross_sections(
                    tin,
                    &align,
                    params.width,
                    params.interval,
                    params.offset_step,
                );
            }
        } else {
            view.sections.clear();
        }
    }
}

fn handle_section_nav(
    prev: Query<&Interaction, (Changed<Interaction>, With<Button>, With<PrevSectionButton>)>,
    next: Query<&Interaction, (Changed<Interaction>, With<Button>, With<NextSectionButton>)>,
    mut view: ResMut<SectionView>,
) {
    if let Ok(&Interaction::Pressed) = prev.get_single() {
        if view.current > 0 {
            view.current -= 1;
        }
    }
    if let Ok(&Interaction::Pressed) = next.get_single() {
        if view.current + 1 < view.sections.len() {
            view.current += 1;
        }
    }
}

fn handle_grade_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<GradeButton>)>,
    tin_res: Res<SurfaceTin>,
    selected: Res<SelectedPoints>,
    points: Query<&Transform, With<CadPoint>>,
    mut info: ResMut<GradeInfo>,
    mut spans: Query<&mut TextSpan>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if let (Some(tin), Some(e)) = (tin_res.0.as_ref(), selected.0.first()) {
            if let Ok(t) = points.get(*e) {
                let start = Point3::new(t.translation.x as f64, t.translation.y as f64, 0.0);
                if let Some(p) = tin.slope_projection(start, (1.0, 0.0), -0.1, 1.0, 50.0) {
                    if let Some(id) = info.text {
                        if let Ok(mut span) = spans.get_mut(id) {
                            span.0 = format!("Grade Result: {:.2},{:.2},{:.2}", p.x, p.y, p.z);
                        }
                    }
                    println!("Daylight at ({:.2}, {:.2}, {:.2})", p.x, p.y, p.z);
                }
            }
        }
    }
}

fn update_alignment_lines(
    data: Res<AlignmentData>,
    points: Query<&Transform, With<CadPoint>>,
    mut commands: Commands,
    existing: Query<Entity, With<AlignmentLine>>,
) {
    if data.is_changed() {
        for e in &existing {
            commands.entity(e).despawn_recursive();
        }
        for pair in data.points.windows(2) {
            if let (Ok(t1), Ok(t2)) = (points.get(pair[0]), points.get(pair[1])) {
                let a = t1.translation;
                let b = t2.translation;
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
                        start: pair[0],
                        end: pair[1],
                    },
                    AlignmentLine,
                ));
            }
        }
    }
}

fn update_profile_lines(
    visible: Res<ProfileVisible>,
    data: Res<AlignmentData>,
    points: Query<&Transform, With<CadPoint>>,
    mut commands: Commands,
    existing: Query<Entity, With<ProfileLine>>,
) {
    for e in &existing {
        commands.entity(e).despawn_recursive();
    }
    if visible.0 {
        let offset = 50.0f32;
        for pair in data.points.windows(2) {
            if let (Ok(t1), Ok(t2)) = (points.get(pair[0]), points.get(pair[1])) {
                let a = Vec2::new(t1.translation.x, t1.translation.y + offset);
                let b = Vec2::new(t2.translation.x, t2.translation.y + offset);
                commands.spawn((
                    SpriteBundle {
                        sprite: Sprite {
                            color: Color::BLUE,
                            custom_size: Some(Vec2::new(a.distance(b), 2.0)),
                            ..default()
                        },
                        transform: Transform::from_translation(((a + b) / 2.0).extend(0.0))
                            .with_rotation(Quat::from_rotation_z((b - a).y.atan2((b - a).x))),
                        ..default()
                    },
                    ProfileLine,
                ));
            }
        }
    }
}

fn update_section_lines(
    visible: Res<SectionsVisible>,
    mut view: ResMut<SectionView>,
    data: Res<AlignmentData>,
    points: Query<&Transform, With<CadPoint>>,
    mut commands: Commands,
    mut spans: Query<&mut TextSpan>,
    existing: Query<Entity, With<SectionLine>>,
) {
    for e in view.entities.drain(..) {
        commands.entity(e).despawn_recursive();
    }
    for e in &existing {
        commands.entity(e).despawn_recursive();
    }
    if !visible.0 || view.sections.is_empty() {
        return;
    }
    let sec = &view.sections[view.current.min(view.sections.len() - 1)];
    let mut pts = Vec::new();
    let mut v_pairs = Vec::new();
    for (i, e) in data.points.iter().enumerate() {
        if let Ok(t) = points.get(*e) {
            pts.push(Point::new(t.translation.x as f64, t.translation.y as f64));
            v_pairs.push((i as f64, t.translation.y as f64));
        }
    }
    use survey_cad::alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
    let hal = HorizontalAlignment::new(pts);
    let val = VerticalAlignment::new(v_pairs);
    let align = Alignment::new(hal, val);
    if let (Some(center), Some(dir), Some(grade)) = (
        align.horizontal.point_at(sec.station),
        align.horizontal.direction_at(sec.station),
        align.vertical.elevation_at(sec.station),
    ) {
        let normal = (-dir.1, dir.0);
        let base = -40.0f32;
        let scale = 5.0f32;
        for pair in sec.points.windows(2) {
            let off_a = (pair[0].x - center.x) * normal.0 + (pair[0].y - center.y) * normal.1;
            let off_b = (pair[1].x - center.x) * normal.0 + (pair[1].y - center.y) * normal.1;
            let elev_a = pair[0].z - grade;
            let elev_b = pair[1].z - grade;
            let a = Vec2::new(off_a as f32 * scale, base + elev_a as f32 * scale);
            let b = Vec2::new(off_b as f32 * scale, base + elev_b as f32 * scale);
            let ent = commands
                .spawn((
                    SpriteBundle {
                        sprite: Sprite {
                            color: Color::rgb(1.0, 1.0, 0.0),
                            custom_size: Some(Vec2::new(a.distance(b).max(1.0), 1.0)),
                            ..default()
                        },
                        transform: Transform::from_translation(((a + b) / 2.0).extend(0.0))
                            .with_rotation(Quat::from_rotation_z((b - a).y.atan2((b - a).x))),
                        ..default()
                    },
                    SectionLine,
                ))
                .id();
            view.entities.push(ent);
        }
    }
    if let Some(id) = view.label {
        if let Ok(mut span) = spans.get_mut(id) {
            span.0 = format!("Station: {:.2}", sec.station);
        }
    }
}
