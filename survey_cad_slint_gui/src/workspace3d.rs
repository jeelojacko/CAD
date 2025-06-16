use bevy::prelude::*;
use bevy::math::primitives::{Cuboid, Sphere};
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
    ));
}

pub fn bevy_app(app: &mut App) {
    // `run_bevy_app_with_slint` already registers `DefaultPlugins`.
    // Only additional plugins specific to this workspace should be added here.
    app.init_resource::<PointEntities>()
        .add_plugins(InfiniteGridPlugin)
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (process_ui_events, send_camera_data));
}

fn process_ui_events(
    receiver: Res<UiEventReceiver>,
    mut camera_q: Query<&mut Transform, With<Camera3d>>,
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
                        commands.entity(e).despawn_recursive();
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

fn send_camera_data(sender: Res<BevyDataSender>, camera_q: Query<&Transform, With<Camera3d>>) {
    if let Ok(transform) = camera_q.single() {
        let _ = sender.0.try_send(BevyData::CameraPosition(transform.translation));
    }
}
