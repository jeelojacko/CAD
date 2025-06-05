#[cfg(feature = "pmetra")]
use bevy::prelude::*;
#[cfg(feature = "pmetra")]
use bevy_pmetra::ParametricBox;

/// Render a simple parametric box using Bevy and bevy_pmetra.
#[cfg(feature = "pmetra")]
pub fn render_box(size: Vec3) {
    let _ = env_logger::builder().is_test(true).try_init();

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ParametricBox { size })
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera3dBundle::default());
            commands.spawn(PointLightBundle {
                point_light: PointLight { intensity: 1500.0, shadows_enabled: true, ..default() },
                transform: Transform::from_xyz(4.0, 8.0, 4.0),
                ..default()
            });
        })
        .run();
}
