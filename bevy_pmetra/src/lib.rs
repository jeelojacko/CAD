use bevy::prelude::*;
use bevy::prelude::{Mesh3d, MeshMaterial3d, Cuboid};

/// Plugin spawning a parametric box mesh.
pub struct ParametricBox {
    pub size: Vec3,
}

impl Plugin for ParametricBox {
    fn build(&self, app: &mut App) {
        let size = self.size;
        app.add_systems(Startup, move |mut commands: Commands,
                                         mut meshes: ResMut<Assets<Mesh>>,
                                         mut materials: ResMut<Assets<StandardMaterial>>| {
            commands.spawn(PbrBundle {
                mesh: Mesh3d::from(meshes.add(Mesh::from(Cuboid::new(size.x, size.y, size.z)))),
                material: MeshMaterial3d::from(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.8, 0.8),
                    ..default()
                })),
                ..default()
            });
        });
    }
}
