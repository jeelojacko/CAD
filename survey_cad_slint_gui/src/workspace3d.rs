use bevy::prelude::*;
use bevy::math::primitives::Cuboid;
use bevy_infinite_grid::{InfiniteGridBundle, InfiniteGridPlugin};

pub fn setup_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
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
    app.add_plugins((DefaultPlugins, InfiniteGridPlugin))
        .add_systems(Startup, setup_scene);
}
