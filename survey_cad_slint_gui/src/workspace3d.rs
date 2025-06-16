use bevy::prelude::*;
use bevy::math::{primitives::{Cuboid, Sphere}, Ray3d};
use bevy::render::{camera::Viewport, view::RenderLayers};
use bevy_infinite_grid::{InfiniteGridBundle, InfiniteGridPlugin};
use crossbeam_channel::{Receiver, Sender};
use survey_cad::geometry::Point3;

/// Input events sent from the Slint UI to Bevy.
#[derive(Debug)]
pub enum UiEvent {
    MouseMove { dx: f32, dy: f32 },
    UpdatePoints(Vec<Point3>),
}

/// Data returned from Bevy to the Slint UI.
#[derive(Debug)]
pub enum BevyData {
    CameraPosition(Vec3),
}

#[derive(Resource)]
pub struct UiEventReceiver(pub Receiver<UiEvent>);

#[derive(Resource)]
pub struct BevyDataSender(pub Sender<BevyData>);

#[derive(Resource, Default)]
pub struct PointEntities(pub Vec<Entity>);

const ORIENTATION_LAYER: usize = 10;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct OrientationCamera;

#[derive(Component)]
pub struct OrientationCube;

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(InfiniteGridBundle::default());
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            ..default()
        },
        Transform::default(),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
        Transform::default(),
        GlobalTransform::default(),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, -5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Z),
        MainCamera,
    ));

    // Orientation widget camera
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1,
            viewport: Some(Viewport {
                physical_position: UVec2::new(480, 0),
                physical_size: UVec2::new(160, 160),
                ..default()
            }),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 3.0)).looking_at(Vec3::ZERO, Vec3::Y),
        OrientationCamera,
        RenderLayers::layer(ORIENTATION_LAYER as usize),
    ));

    // Orientation cube
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(materials.add(Color::srgb(0.6, 0.6, 0.6))),
        Transform::default(),
        GlobalTransform::default(),
        OrientationCube,
        RenderLayers::layer(ORIENTATION_LAYER as usize),
    ));
}

pub fn bevy_app(app: &mut App) {
    // `run_bevy_app_with_slint` already registers `DefaultPlugins`.
    // Only additional plugins specific to this workspace should be added here.
    app.init_resource::<PointEntities>()
        .add_plugins(InfiniteGridPlugin)
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                process_ui_events,
                send_camera_data,
                sync_orientation_cube,
                handle_orientation_cube_clicks,
            ),
        );
}

fn process_ui_events(
    receiver: Res<UiEventReceiver>,
    mut camera_q: Query<&mut Transform, (With<Camera3d>, With<MainCamera>)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut point_entities: ResMut<PointEntities>,
) {
    if let Ok(mut transform) = camera_q.single_mut() {
        for event in receiver.0.try_iter() {
            match event {
                UiEvent::MouseMove { dx, dy } => {
                    transform.translation.x += dx * 0.01;
                    transform.translation.y -= dy * 0.01;
                }
                UiEvent::UpdatePoints(pts) => {
                    for e in point_entities.0.drain(..) {
                        commands.entity(e).despawn();
                    }
                    for p in pts {
                        let e = commands
                            .spawn((
                                Mesh3d(meshes.add(Sphere::new(0.1))),
                                MeshMaterial3d(materials.add(Color::srgb(0.9, 0.1, 0.1))),
                                Transform::from_xyz(p.x as f32, p.y as f32, p.z as f32),
                                GlobalTransform::default(),
                            ))
                            .id();
                        point_entities.0.push(e);
                    }
                }
            }
        }
    }
}

fn send_camera_data(
    sender: Res<BevyDataSender>,
    camera_q: Query<&Transform, (With<Camera3d>, With<MainCamera>)>,
) {
    if let Ok(transform) = camera_q.single() {
        let _ = sender.0.try_send(BevyData::CameraPosition(transform.translation));
    }
}

fn sync_orientation_cube(
    main_cam: Query<&Transform, (With<Camera3d>, With<MainCamera>)>,
    mut cube: Query<&mut Transform, With<OrientationCube>>,
) {
    if let (Ok(cam), Ok(mut cube_tf)) = (main_cam.single(), cube.single_mut()) {
        cube_tf.rotation = cam.rotation;
    }
}

fn ray_cube_intersection(ray: Ray3d) -> Option<Vec3> {
    let mut closest = f32::INFINITY;
    let mut normal = None;
    let checks = [
        (Vec3::X, 0.5),
        (Vec3::NEG_X, -0.5),
        (Vec3::Y, 0.5),
        (Vec3::NEG_Y, -0.5),
        (Vec3::Z, 0.5),
        (Vec3::NEG_Z, -0.5),
    ];
    let dir: Vec3 = ray.direction.into();
    for (axis, plane) in checks {
        let denom = axis.dot(dir);
        if denom.abs() < 1e-6 {
            continue;
        }
        let t = (plane - axis.dot(ray.origin)) / denom;
        if t < 0.0 || t >= closest {
            continue;
        }
        let hit = ray.origin + dir * t;
        if hit.x.abs() <= 0.5 + 1e-6 && hit.y.abs() <= 0.5 + 1e-6 && hit.z.abs() <= 0.5 + 1e-6 {
            closest = t;
            normal = Some(axis);
        }
    }
    normal
}

fn handle_orientation_cube_clicks(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    ori_cam_q: Query<(&Camera, &GlobalTransform), With<OrientationCamera>>,
    cube_q: Query<&GlobalTransform, With<OrientationCube>>,
    mut main_cam_q: Query<&mut Transform, (With<Camera3d>, With<MainCamera>)>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let window = if let Some(w) = windows.iter().next() { w } else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok((camera, cam_tf)) = ori_cam_q.single() else { return };
    let Some(viewport) = &camera.viewport else { return };
    let pos = cursor - viewport.physical_position.as_vec2();
    if pos.x < 0.0
        || pos.y < 0.0
        || pos.x > viewport.physical_size.x as f32
        || pos.y > viewport.physical_size.y as f32
    {
        return;
    }
    let Ok(ray) = camera.viewport_to_world(cam_tf, pos) else { return };
    if let Some(local_normal) = ray_cube_intersection(ray) {
        let Ok(cube_tf) = cube_q.single() else { return };
        let dir_world = cube_tf.rotation() * local_normal;
        if let Ok(mut cam) = main_cam_q.single_mut() {
            let dist = cam.translation.length();
            cam.translation = -dir_world.normalize() * dist;
            let up = if dir_world.z.abs() > 0.9 { Vec3::Y } else { Vec3::Z };
            cam.look_at(Vec3::ZERO, up);
        }
    }
}
