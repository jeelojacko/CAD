use bevy::prelude::*;
use bevy::render::mesh::shape;
use bevy_editor_cam::{DefaultEditorCamPlugins, controller::component::EditorCam};

pub fn setup_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 10000.0,
            ..default()
        },
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
        material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
        ..default()
    });

    commands.spawn((Camera3d::default(), EditorCam::default()));
}

pub fn bevy_app(app: &mut App) {
    app.add_plugins(DefaultPlugins)
        .add_plugins(DefaultEditorCamPlugins)
        .add_systems(Startup, setup_scene);
}
