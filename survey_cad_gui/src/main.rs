use bevy::prelude::*;
use survey_cad::geometry::Point;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Survey CAD GUI".into(),
                resolution: (800.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
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
    commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: Color::srgb(1.0, 0.0, 0.0),
            custom_size: Some(Vec2::splat(5.0)),
            ..default()
        },
        transform: Transform::from_translation(Vec3::new(p.x as f32, p.y as f32, 0.0)),
        ..default()
    });
}
