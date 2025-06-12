#![allow(clippy::type_complexity, clippy::too_many_arguments)]
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::log::warn;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy_editor_cam::prelude::*;
use clap::{Parser, ValueEnum};
use std::collections::HashMap;
use std::fs::File;
use survey_cad::geometry::{Point, Point3, Polyline};
use survey_cad::{crs::Crs, geometry::distance};

#[derive(Copy, Clone, ValueEnum)]
enum WorkspaceProfile {
    Surveyor,
    Engineer,
    Gis,
}

#[derive(Copy, Clone, ValueEnum)]
enum Theme {
    Dark,
    Light,
}

#[derive(Parser)]
struct Args {
    /// EPSG code for the working coordinate system
    #[arg(long, default_value_t = 4326)]
    epsg: u32,
    /// Workspace profile (surveyor, engineer, gis)
    #[arg(long, value_enum, default_value_t = WorkspaceProfile::Surveyor)]
    profile: WorkspaceProfile,
    /// UI theme (dark or light)
    #[arg(long, value_enum, default_value_t = Theme::Dark)]
    theme: Theme,
}

#[derive(Resource, Default)]
struct SelectedPoints(Vec<Entity>);

#[derive(Resource, Default)]
struct Dragging(Option<Entity>);

#[derive(Resource, Default)]
struct SelectMode(bool);

#[derive(Resource, Default)]
struct DragSelect {
    active: bool,
    start: Vec2,
    end: Vec2,
}

#[derive(Component)]
struct CadPoint;

#[derive(Component)]
struct CadLine {
    start: Entity,
    end: Entity,
}

#[derive(Resource)]
struct WorkingCrs(Crs);

#[derive(Resource)]
struct CurrentProfile(WorkspaceProfile);
#[derive(Resource)]
struct ThemeColors {
    toolbar_bg: Color,
    button_bg: Color,
    panel_bg: Color,
    context_bg: Color,
    text: Color,
}

impl ThemeColors {
    fn new(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self {
                toolbar_bg: Color::srgb(0.2, 0.2, 0.2),
                button_bg: Color::srgb(0.3, 0.3, 0.3),
                panel_bg: Color::srgb(0.15, 0.15, 0.15),
                context_bg: Color::srgb(0.2, 0.2, 0.2),
                text: Color::WHITE,
            },
            Theme::Light => Self {
                toolbar_bg: Color::srgb(0.9, 0.9, 0.9),
                button_bg: Color::srgb(0.8, 0.8, 0.8),
                panel_bg: Color::srgb(0.95, 0.95, 0.95),
                context_bg: Color::srgb(0.9, 0.9, 0.9),
                text: Color::BLACK,
            },
        }
    }
}

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
struct SurfaceDirty(bool);

#[derive(Component)]
struct BreaklineEdge;

#[derive(Component)]
struct HoleEdge;

#[derive(Resource, Default)]
struct SurfaceTins(Vec<survey_cad::dtm::Tin>);

#[derive(Component)]
struct SurfaceMesh;

#[derive(Component)]
struct LevelOfDetail {
    high: Handle<Mesh>,
    low: Handle<Mesh>,
    threshold: f32,
}

#[derive(Component)]
struct BuildSurfaceButton;

#[derive(Component)]
struct ShowProfileButton;

#[derive(Component)]
struct ShowSectionsButton;

#[derive(Component)]
struct ShowPlanButton;

#[derive(Component)]
struct PlanLabel;

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
struct SelectButton;

#[derive(Component)]
struct GradeButton;

#[derive(Component)]
struct OpenButton;

#[derive(Component)]
struct SaveButton;

#[derive(Component)]
struct NewButton;

#[derive(Component)]
struct FileMenuButton;

#[derive(Resource, Default)]
struct FileMenuState {
    entity: Option<Entity>,
}

#[derive(Component)]
struct CogoMenuButton;

#[derive(Resource, Default)]
struct CogoMenuState {
    entity: Option<Entity>,
}

#[derive(Component)]
struct SurfaceMenuButton;

#[derive(Resource, Default)]
struct SurfaceMenuState {
    entity: Option<Entity>,
}

#[derive(Component)]
struct CogoButton(CogoAction);

#[derive(Clone, Copy)]
enum CogoAction {
    Bearing,
    Forward,
    Intersection,
    LevelElevation,
    VerticalAngle,
}

#[derive(Component)]
struct CrsMenuButton;

#[derive(Resource, Default)]
struct CrsMenuState {
    entity: Option<Entity>,
}

#[derive(Resource)]
struct CrsDatabase(Vec<survey_cad::crs::CrsEntry>);

#[derive(Component, Clone)]
struct CrsOption(String);

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
struct PlanVisible(bool);

#[derive(Resource, Default, Clone, Copy)]
enum WorkspaceMode {
    #[default]
    TwoD,
    ThreeD,
}

#[derive(Component)]
struct ViewToggleButton;

#[derive(Component)]
struct WorkspaceCamera2d;

#[derive(Component)]
struct WorkspaceCamera3d;

#[derive(Resource, Default)]
struct ContextMenuState {
    entity: Option<Entity>,
}

#[derive(Component)]
struct ContextButton(ContextAction);

#[derive(Clone, Copy)]
enum ContextAction {
    DeletePoints,
    RaiseElevation,
    LowerElevation,
}

#[derive(Resource, Default)]
struct ParcelData {
    parcels: Vec<survey_cad::parcel::Parcel>,
    text: Option<Entity>,
}

#[derive(Resource, Default)]
struct GradeInfo {
    text: Option<Entity>,
}

#[derive(Resource)]
struct SectionView {
    sections: Vec<survey_cad::corridor::CrossSection>,
    design: Vec<survey_cad::corridor::CrossSection>,
    current: usize,
    station: f64,
    scale: f32,
    exaggeration: f32,
    show_ground: bool,
    show_design: bool,
    entities: Vec<Entity>,
    label: Option<Entity>,
}

impl Default for SectionView {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            design: Vec::new(),
            current: 0,
            station: 0.0,
            scale: 5.0,
            exaggeration: 1.0,
            show_ground: true,
            show_design: true,
            entities: Vec::new(),
            label: None,
        }
    }
}

#[derive(Component)]
struct PrevSectionButton;

#[derive(Component)]
struct NextSectionButton;

#[derive(Component)]
struct SectionButton(SectionControl);

#[derive(Clone, Copy)]
enum SectionControl {
    ScaleInc,
    ScaleDec,
    ExagInc,
    ExagDec,
    ToggleGround,
    ToggleDesign,
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
    if let Ok(path) = std::env::var("SURVEY_CAD_LOG") {
        match File::create(&path) {
            Ok(file) => {
                env_logger::Builder::from_default_env()
                    .target(env_logger::Target::Pipe(Box::new(file)))
                    .init();
            }
            Err(e) => {
                eprintln!("Failed to create log file {}: {}", path, e);
                env_logger::Builder::from_default_env().init();
            }
        }
    } else {
        env_logger::Builder::from_default_env().init();
    }

    let args = Args::parse();
    println!("Using EPSG {}", args.epsg);
    App::new()
        .insert_resource(WorkingCrs(Crs::from_epsg(args.epsg)))
        .insert_resource(CurrentProfile(args.profile))
        .insert_resource(ThemeColors::new(args.theme))
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Survey CAD GUI".into(),
                    resolution: (800.0, 600.0).into(),
                    ..default()
                }),
                ..default()
            }),
            DefaultEditorCamPlugins,
            bevy_gizmos::GizmoPlugin,
        ))
        .insert_resource(SelectedPoints::default())
        .insert_resource(Dragging::default())
        .insert_resource(SelectMode::default())
        .insert_resource(DragSelect::default())
        .insert_resource(AlignmentData::default())
        .insert_resource(SurfaceData::default())
        .insert_resource(SurfaceTins::default())
        .insert_resource(SurfaceDirty::default())
        .insert_resource(CorridorParams::default())
        .insert_resource(ProfileVisible::default())
        .insert_resource(SectionsVisible::default())
        .insert_resource(PlanVisible::default())
        .insert_resource(WorkspaceMode::default())
        .insert_resource(ContextMenuState::default())
        .insert_resource(CogoMenuState::default())
        .insert_resource(SurfaceMenuState::default())
        .insert_resource(FileMenuState::default())
        .insert_resource(CrsMenuState::default())
        .insert_resource(CrsDatabase(survey_cad::crs::list_known_crs()))
        .add_systems(Startup, (setup, init_ui_scale))
        .add_systems(
            Update,
            (
                handle_mouse_clicks,
                open_context_menu,
                handle_context_menu_buttons,
                drag_point,
                create_line,
                update_lines,
                update_alignment_lines,
                handle_add_alignment,
                handle_add_surface,
                handle_add_breakline,
                handle_add_hole,
                handle_point_elevation,
                update_surface_edges,
                maybe_update_surface,
                camera_pan_zoom,
                handle_view_toggle,
                highlight_selected_points,
                update_lod_meshes,
            ),
        )
        .add_systems(
            Update,
            (
                handle_add_parcel,
                handle_corridor_buttons,
                handle_build_surface,
                handle_grade_button,
                handle_select_button,
                handle_file_menu_button,
                handle_cogo_menu_button,
                handle_surface_menu_button,
                handle_crs_menu_button,
                handle_new_button,
                handle_open_button,
                handle_save_button,
                handle_cogo_buttons,
                handle_show_plan,
                handle_show_profile,
                handle_show_sections,
                handle_section_nav,
                handle_section_buttons,
                update_profile_lines,
                update_plan_labels,
                update_section_lines,
                handle_crs_option_buttons,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    working: Res<WorkingCrs>,
    profile: Res<CurrentProfile>,
    theme: Res<ThemeColors>,
) {
    println!("GUI working CRS: {}", working.0.definition());
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        WorkspaceCamera2d,
    ));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 0,
            is_active: false,
            ..default()
        },
        Transform::from_xyz(0.0, -50.0, 50.0).looking_at(Vec3::ZERO, Vec3::Z),
        EditorCam::default(),
        WorkspaceCamera3d,
    ));
    commands.spawn((
        DirectionalLight {
            shadows_enabled: false,
            ..default()
        },
        Transform::default(),
    ));
    spawn_toolbar(&mut commands, &asset_server, profile.0, &theme);
    spawn_file_toolbar(&mut commands, &asset_server, &theme);
    let (parcel_text, grade_text) =
        spawn_edit_panel(&mut commands, &asset_server, profile.0, &theme);
    let section_label = spawn_sections_panel(&mut commands, &asset_server, &theme);
    commands.insert_resource(ParcelData {
        parcels: Vec::new(),
        text: Some(parcel_text),
    });
    commands.insert_resource(GradeInfo {
        text: Some(grade_text),
    });
    commands.insert_resource(SectionView {
        label: Some(section_label),
        ..Default::default()
    });
    // Example content
    let _ = spawn_point(&mut commands, Point::new(0.0, 0.0));
}

fn spawn_toolbar(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    profile: WorkspaceProfile,
    theme: &ThemeColors,
) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.toolbar_bg),
        ))
        .insert(FocusPolicy::Block)
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|button| {
                    button.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("File"),
                    ));
                })
                .insert(FileMenuButton);

            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|button| {
                    button.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("Edit"),
                    ));
                });

            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|button| {
                    button.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("View"),
                    ));
                })
                .insert(ViewToggleButton);

            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|button| {
                    button.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("Cogo"),
                    ));
                })
                .insert(CogoMenuButton);

            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|button| {
                    button.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("Surface"),
                    ));
                })
                .insert(SurfaceMenuButton);

            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|button| {
                    button.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("CRS"),
                    ));
                })
                .insert(CrsMenuButton);

            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|button| {
                    let label = match profile {
                        WorkspaceProfile::Surveyor => "Survey",
                        WorkspaceProfile::Engineer => "Engineering",
                        WorkspaceProfile::Gis => "GIS",
                    };
                    button.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new(label),
                    ));
                });

            // Additional file controls are provided by a secondary toolbar
        });
}

fn spawn_file_toolbar(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    theme: &ThemeColors,
) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(30.0),
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.toolbar_bg),
        ))
        .insert(FocusPolicy::Block)
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|b| {
                    b.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("New"),
                    ));
                })
                .insert(NewButton);

            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|b| {
                    b.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("Open"),
                    ));
                })
                .insert(OpenButton);

            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::new(
                            Val::Px(10.0),
                            Val::Px(10.0),
                            Val::Px(5.0),
                            Val::Px(5.0),
                        ),
                        ..default()
                    },
                    BackgroundColor(theme.button_bg),
                ))
                .with_children(|b| {
                    b.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(theme.text),
                        Text::new("Save"),
                    ));
                })
                .insert(SaveButton);
        });
}

fn spawn_edit_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    profile: WorkspaceProfile,
    theme: &ThemeColors,
) -> (Entity, Entity) {
    let mut parcel_text = Entity::from_raw(0);
    let mut grade_text = Entity::from_raw(0);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(60.0),
                width: Val::Px(200.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            BackgroundColor(theme.panel_bg),
        ))
        .insert(FocusPolicy::Block)
        .with_children(|parent| {
            if matches!(
                profile,
                WorkspaceProfile::Surveyor | WorkspaceProfile::Engineer
            ) {
                parent.spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("FiraMono-subset.ttf"),
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    Text::new("Alignment Editor"),
                ));
                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Add Selected"),
                        ));
                    })
                    .insert(AddAlignmentButton);
            }

            parent
                .spawn(Button)
                .with_children(|b| {
                    b.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor::WHITE,
                        Text::new("Select Mode"),
                    ));
                })
                .insert(SelectButton);

            parent.spawn((
                TextLayout::default(),
                TextFont {
                    font: asset_server.load("FiraMono-subset.ttf"),
                    font_size: 14.0,
                    ..default()
                },
                TextColor::WHITE,
                Text::new("Surface Editor"),
            ));
            if matches!(profile, WorkspaceProfile::Surveyor) {
                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Add Points"),
                        ));
                    })
                    .insert(AddSurfaceButton);
            }

            if matches!(profile, WorkspaceProfile::Surveyor) {
                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Add Breakline"),
                        ));
                    })
                    .insert(AddBreaklineButton);
            }

            if matches!(profile, WorkspaceProfile::Surveyor) {
                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Add Hole"),
                        ));
                    })
                    .insert(AddHoleButton);
            }

            if matches!(profile, WorkspaceProfile::Surveyor | WorkspaceProfile::Gis) {
                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Add Parcel"),
                        ));
                    })
                    .insert(AddParcelButton);
            }

            parcel_text = parent
                .spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("FiraMono-subset.ttf"),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    Text::new("Parcel Area (Selected): 0"),
                ))
                .id();

            grade_text = parent
                .spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("FiraMono-subset.ttf"),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    Text::new("Grade Result:"),
                ))
                .id();

            if matches!(
                profile,
                WorkspaceProfile::Surveyor | WorkspaceProfile::Engineer
            ) {
                parent.spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("FiraMono-subset.ttf"),
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    Text::new("Corridor Params"),
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
                        .spawn(Button)
                        .with_children(|b| {
                            b.spawn((
                                TextLayout::default(),
                                TextFont {
                                    font: asset_server.load("FiraMono-subset.ttf"),
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor::WHITE,
                                Text::new(label),
                            ));
                        })
                        .insert(CorridorButton(ctl));
                }

                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Build Surface"),
                        ));
                    })
                    .insert(BuildSurfaceButton);

                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Grade Slope"),
                        ));
                    })
                    .insert(GradeButton);
            }

            if matches!(
                profile,
                WorkspaceProfile::Surveyor | WorkspaceProfile::Engineer
            ) {
                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Show Profile"),
                        ));
                    })
                    .insert(ShowProfileButton);

                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new("Show Sections"),
                        ));
                    })
                    .insert(ShowSectionsButton);
            }

            parent
                .spawn(Button)
                .with_children(|b| {
                    b.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor::WHITE,
                        Text::new("Show Plan"),
                    ));
                })
                .insert(ShowPlanButton);
        });
    (parcel_text, grade_text)
}

fn spawn_sections_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    theme: &ThemeColors,
) -> Entity {
    let mut label = Entity::from_raw(0);
    commands
        .spawn((
            Node {
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
            BackgroundColor(theme.panel_bg),
        ))
        .insert(FocusPolicy::Block)
        .with_children(|parent| {
            parent
                .spawn(Button)
                .with_children(|b| {
                    b.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor::WHITE,
                        Text::new("Prev"),
                    ));
                })
                .insert(PrevSectionButton);
            label = parent
                .spawn((
                    TextLayout::default(),
                    TextFont {
                        font: asset_server.load("FiraMono-subset.ttf"),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor::WHITE,
                    Text::new("Station: 0.0"),
                ))
                .id();
            parent
                .spawn(Button)
                .with_children(|b| {
                    b.spawn((
                        TextLayout::default(),
                        TextFont {
                            font: asset_server.load("FiraMono-subset.ttf"),
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor::WHITE,
                        Text::new("Next"),
                    ));
                })
                .insert(NextSectionButton);

            for (label, ctl) in [
                ("Scale -", SectionControl::ScaleDec),
                ("Scale +", SectionControl::ScaleInc),
                ("Exag -", SectionControl::ExagDec),
                ("Exag +", SectionControl::ExagInc),
                ("Ground", SectionControl::ToggleGround),
                ("Design", SectionControl::ToggleDesign),
            ] {
                parent
                    .spawn(Button)
                    .with_children(|b| {
                        b.spawn((
                            TextLayout::default(),
                            TextFont {
                                font: asset_server.load("FiraMono-subset.ttf"),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor::WHITE,
                            Text::new(label),
                        ));
                    })
                    .insert(SectionButton(ctl));
            }
        });
    label
}

fn spawn_point(commands: &mut Commands, p: Point) -> Entity {
    commands
        .spawn((
            Sprite {
                color: Color::srgb(1.0, 0.0, 0.0),
                custom_size: Some(Vec2::splat(5.0)),
                ..default()
            },
            Transform::from_translation(Vec3::new(p.x as f32, p.y as f32, 0.0)),
            CadPoint,
        ))
        .id()
}

fn cursor_world_pos(
    windows: &Query<&Window>,
    camera_q: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
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
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    points: Query<(Entity, &Transform), With<CadPoint>>,
    mut selected: ResMut<SelectedPoints>,
    mut dragging: ResMut<Dragging>,
    mode: Res<SelectMode>,
    mut drag_box: ResMut<DragSelect>,
    ui_buttons: Query<&Interaction, With<Button>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        if ui_buttons.iter().any(|i| *i != Interaction::None) {
            return;
        }
        if let Some(pos) = cursor_world_pos(&windows, &camera_q) {
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
            } else if !mode.0 {
                let _ = spawn_point(&mut commands, Point::new(pos.x as f64, pos.y as f64));
            } else {
                drag_box.active = true;
                drag_box.start = pos;
                drag_box.end = pos;
            }
        }
    }

    if buttons.pressed(MouseButton::Left) && drag_box.active {
        if let Some(pos) = cursor_world_pos(&windows, &camera_q) {
            drag_box.end = pos;
        }
    }

    if buttons.just_released(MouseButton::Left) {
        dragging.0 = None;
        if drag_box.active {
            drag_box.active = false;
            if cursor_world_pos(&windows, &camera_q).is_some() {
                let min_x = drag_box.start.x.min(drag_box.end.x);
                let max_x = drag_box.start.x.max(drag_box.end.x);
                let min_y = drag_box.start.y.min(drag_box.end.y);
                let max_y = drag_box.start.y.max(drag_box.end.y);
                selected.0.clear();
                for (e, t) in &points {
                    let p = t.translation.truncate();
                    if p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y {
                        selected.0.push(e);
                    }
                }
            }
        }
    }
}

fn open_context_menu(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    asset_server: Res<AssetServer>,
    mut state: ResMut<ContextMenuState>,
    selected: Res<SelectedPoints>,
    theme: Res<ThemeColors>,
) {
    if buttons.just_pressed(MouseButton::Right) && !selected.0.is_empty() {
        if let Some(pos) = windows.single().cursor_position() {
            if let Some(e) = state.entity.take() {
                commands.entity(e).despawn_recursive();
            }
            let height = windows.single().height();
            let menu = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(pos.x),
                        bottom: Val::Px(height - pos.y),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(theme.context_bg),
                ))
                .insert(FocusPolicy::Block)
                .with_children(|parent| {
                    for (label, action) in [
                        ("Delete", ContextAction::DeletePoints),
                        ("Raise 0.1", ContextAction::RaiseElevation),
                        ("Lower 0.1", ContextAction::LowerElevation),
                    ] {
                        parent
                            .spawn(Button)
                            .with_children(|b| {
                                b.spawn((
                                    TextLayout::default(),
                                    TextFont {
                                        font: asset_server.load("FiraMono-subset.ttf"),
                                        font_size: 12.0,
                                        ..default()
                                    },
                                    TextColor::WHITE,
                                    Text::new(label),
                                ));
                            })
                            .insert(ContextButton(action));
                    }
                })
                .id();
            state.entity = Some(menu);
        }
    }
    if state.entity.is_some() && buttons.just_pressed(MouseButton::Left) {
        if let Some(e) = state.entity.take() {
            commands.entity(e).despawn_recursive();
        }
    }
}

fn drag_point(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut points: Query<&mut Transform, With<CadPoint>>,
    dragging: Res<Dragging>,
    mut data: ResMut<SurfaceData>,
    mut dirty: ResMut<SurfaceDirty>,
) {
    if let Some(e) = dragging.0 {
        if buttons.pressed(MouseButton::Left) {
            if let Some(pos) = cursor_world_pos(&windows, &camera_q) {
                if let Ok(mut t) = points.get_mut(e) {
                    t.translation.x = pos.x;
                    t.translation.y = pos.y;
                    if let Some(&idx) = data.point_map.get(&e) {
                        if let Some(v) = data.vertices.get_mut(idx) {
                            v.x = pos.x as f64;
                            v.y = pos.y as f64;
                            dirty.0 = true;
                            data.set_changed();
                        }
                    }
                }
            }
        }
    }
}

fn camera_pan_zoom(
    mut camera_q: Query<(&mut Transform, &mut OrthographicProjection), With<Camera2d>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion_evr: EventReader<MouseMotion>,
    mut wheel_evr: EventReader<MouseWheel>,
    menu: Res<ContextMenuState>,
) {
    let (mut transform, mut projection) = camera_q.single_mut();
    for ev in wheel_evr.read() {
        let factor = 1.0 - ev.y * 0.1;
        projection.scale = (projection.scale * factor).clamp(0.1, 1000.0);
    }
    if menu.entity.is_none() && buttons.pressed(MouseButton::Right) {
        for ev in motion_evr.read() {
            transform.translation.x -= ev.delta.x * projection.scale;
            transform.translation.y += ev.delta.y * projection.scale;
        }
    }
}

fn handle_context_menu_buttons(
    interactions: Query<(&Interaction, &ContextButton), Changed<Interaction>>,
    mut commands: Commands,
    mut selected: ResMut<SelectedPoints>,
    mut points: Query<&mut Transform, With<CadPoint>>,
    mut data: ResMut<SurfaceData>,
    mut dirty: ResMut<SurfaceDirty>,
    mut menu: ResMut<ContextMenuState>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed {
            match button.0 {
                ContextAction::DeletePoints => {
                    for e in selected.0.drain(..) {
                        commands.entity(e).despawn_recursive();
                    }
                }
                ContextAction::RaiseElevation => {
                    for &e in &selected.0 {
                        if let Ok(mut t) = points.get_mut(e) {
                            t.translation.z += 0.1;
                            if let Some(&idx) = data.point_map.get(&e) {
                                if let Some(v) = data.vertices.get_mut(idx) {
                                    v.z = t.translation.z as f64;
                                    dirty.0 = true;
                                    data.set_changed();
                                }
                            }
                        }
                    }
                }
                ContextAction::LowerElevation => {
                    for &e in &selected.0 {
                        if let Ok(mut t) = points.get_mut(e) {
                            t.translation.z -= 0.1;
                            if let Some(&idx) = data.point_map.get(&e) {
                                if let Some(v) = data.vertices.get_mut(idx) {
                                    v.z = t.translation.z as f64;
                                    dirty.0 = true;
                                    data.set_changed();
                                }
                            }
                        }
                    }
                }
            }
            if let Some(ent) = menu.entity.take() {
                commands.entity(ent).despawn_recursive();
            }
        }
    }
}

fn handle_point_elevation(
    keys: Res<ButtonInput<KeyCode>>,
    selected: Res<SelectedPoints>,
    mut points: Query<&mut Transform, With<CadPoint>>,
    mut data: ResMut<SurfaceData>,
    mut dirty: ResMut<SurfaceDirty>,
) {
    let mut delta = 0.0;
    if keys.pressed(KeyCode::ArrowUp) {
        delta += 0.1;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        delta -= 0.1;
    }
    if delta != 0.0 {
        for e in &selected.0 {
            if let Ok(mut t) = points.get_mut(*e) {
                t.translation.z += delta;
                if let Some(&idx) = data.point_map.get(e) {
                    if let Some(v) = data.vertices.get_mut(idx) {
                        v.z = t.translation.z as f64;
                        dirty.0 = true;
                        data.set_changed();
                    }
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
        if let (Ok(a), Ok(b)) = (points.get(selected.0[0]), points.get(selected.0[1])) {
            let a = a.translation;
            let b = b.translation;
            commands.spawn((
                Sprite {
                    color: Color::WHITE,
                    custom_size: Some(Vec2::new(a.distance(b), 2.0)),
                    ..default()
                },
                Transform::from_translation((a + b) / 2.0)
                    .with_rotation(Quat::from_rotation_z((b - a).y.atan2((b - a).x))),
                CadLine {
                    start: selected.0[0],
                    end: selected.0[1],
                },
            ));
        } else {
            warn!("cannot create line; missing selected points");
        }
    }
}

fn update_lines(
    mut lines: Query<(&CadLine, &mut Transform, &mut Sprite), Without<CadPoint>>,
    points: Query<&Transform, With<CadPoint>>,
) {
    for (line, mut t, mut s) in &mut lines {
        if let (Ok(a), Ok(b)) = (points.get(line.start), points.get(line.end)) {
            let a = a.translation;
            let b = b.translation;
            s.custom_size = Some(Vec2::new(a.distance(b), 2.0));
            t.translation = (a + b) / 2.0;
            t.rotation = Quat::from_rotation_z((b - a).y.atan2((b - a).x));
        } else {
            warn!("skipping line update; point entity not found");
        }
    }
}

fn highlight_selected_points(
    mut points: Query<(Entity, &mut Sprite), With<CadPoint>>,
    selected: Res<SelectedPoints>,
) {
    for (e, mut sprite) in &mut points {
        if selected.0.contains(&e) {
            sprite.color = Color::srgb(0.2, 0.6, 1.0);
        } else {
            sprite.color = Color::srgb(1.0, 0.0, 0.0);
        }
    }
}

fn update_surface_edges(
    data: Res<SurfaceData>,
    mut commands: Commands,
    existing: Query<Entity, Or<(With<BreaklineEdge>, With<HoleEdge>)>>,
) {
    if data.is_changed() {
        for e in &existing {
            commands.entity(e).despawn_recursive();
        }
        for (i1, i2) in &data.breaklines {
            if let (Some(a), Some(b)) = (data.vertices.get(*i1), data.vertices.get(*i2)) {
                let va = Vec2::new(a.x as f32, a.y as f32);
                let vb = Vec2::new(b.x as f32, b.y as f32);
                commands
                    .spawn((
                        Sprite {
                            color: Color::srgb(1.0, 0.5, 0.0),
                            custom_size: Some(Vec2::new(va.distance(vb), 2.0)),
                            ..default()
                        },
                        Transform::from_translation(((va + vb) / 2.0).extend(0.0))
                            .with_rotation(Quat::from_rotation_z((vb - va).y.atan2((vb - va).x))),
                    ))
                    .insert(BreaklineEdge);
            }
        }
        for hole in &data.holes {
            for i in 0..hole.len() {
                let i1 = hole[i];
                let i2 = hole[(i + 1) % hole.len()];
                if let (Some(a), Some(b)) = (data.vertices.get(i1), data.vertices.get(i2)) {
                    let va = Vec2::new(a.x as f32, a.y as f32);
                    let vb = Vec2::new(b.x as f32, b.y as f32);
                    commands
                        .spawn((
                            Sprite {
                                color: Color::srgb(0.5, 0.0, 0.5),
                                custom_size: Some(Vec2::new(va.distance(vb), 2.0)),
                                ..default()
                            },
                            Transform::from_translation(((va + vb) / 2.0).extend(0.0))
                                .with_rotation(Quat::from_rotation_z(
                                    (vb - va).y.atan2((vb - va).x),
                                )),
                        ))
                        .insert(HoleEdge);
                }
            }
        }
    }
}

fn maybe_update_surface(
    mut dirty_flag: ResMut<SurfaceDirty>,
    data: Res<SurfaceData>,
    mut tin_res: ResMut<SurfaceTins>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<SurfaceMesh>>,
) {
    if dirty_flag.0 {
        for e in &existing {
            commands.entity(e).despawn_recursive();
        }
        if let Some(tin) = build_tin(&data) {
            let high_mesh = build_surface_mesh(&tin);
            let low_mesh = build_lowres_surface_mesh(&tin);
            let handle = meshes.add(high_mesh);
            let low_handle = meshes.add(low_mesh);
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.0, 1.0, 0.0),
                ..default()
            });
            commands
                .spawn((Mesh3d(handle.clone()), MeshMaterial3d(mat)))
                .insert(SurfaceMesh)
                .insert(LevelOfDetail {
                    high: handle.clone(),
                    low: low_handle.clone(),
                    threshold: 2.0,
                });
            tin_res.0.push(tin);
        }
        dirty_flag.0 = false;
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
    mut dirty: ResMut<SurfaceDirty>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
    mut menu: ResMut<SurfaceMenuState>,
    mut commands: Commands,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        for e in &selected.0 {
            if let Ok(t) = points.get(*e) {
                let idx = data.vertices.len();
                data.vertices.push(Point3::new(
                    t.translation.x as f64,
                    t.translation.y as f64,
                    t.translation.z as f64,
                ));
                data.point_map.insert(*e, idx);
                dirty.0 = true;
                data.set_changed();
            }
        }
        if let Some(ent) = menu.entity.take() {
            commands.entity(ent).despawn_recursive();
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
            t.translation.z as f64,
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
    mut dirty: ResMut<SurfaceDirty>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
    mut menu: ResMut<SurfaceMenuState>,
    mut commands: Commands,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if selected.0.len() >= 2 {
            let a = selected.0[0];
            let b = selected.0[1];
            if let (Ok(t1), Ok(t2)) = (points.get(a), points.get(b)) {
                let i1 = get_vertex_index(&mut data, a, t1);
                let i2 = get_vertex_index(&mut data, b, t2);
                if let Some(pos) = data
                    .breaklines
                    .iter()
                    .position(|&(x, y)| (x == i1 && y == i2) || (x == i2 && y == i1))
                {
                    data.breaklines.remove(pos);
                    println!("Removed breakline between {} and {}", i1, i2);
                } else {
                    data.breaklines.push((i1, i2));
                    println!("Added breakline between {} and {}", i1, i2);
                }
                dirty.0 = true;
                data.set_changed();
            }
        }
        if let Some(ent) = menu.entity.take() {
            commands.entity(ent).despawn_recursive();
        }
    }
}

fn handle_add_hole(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<AddHoleButton>)>,
    mut data: ResMut<SurfaceData>,
    mut dirty: ResMut<SurfaceDirty>,
    points: Query<&Transform, With<CadPoint>>,
    selected: Res<SelectedPoints>,
    mut menu: ResMut<SurfaceMenuState>,
    mut commands: Commands,
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
            if let Some(pos) = data.holes.iter().position(|h| *h == hole) {
                data.holes.remove(pos);
                println!("Removed hole with {} vertices", hole.len());
            } else {
                data.holes.push(hole);
                let len = data.holes.last().map(|h| h.len()).unwrap_or(0);
                println!("Added hole with {} vertices", len);
            }
            dirty.0 = true;
            data.set_changed();
        }
        if let Some(ent) = menu.entity.take() {
            commands.entity(ent).despawn_recursive();
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
                    span.0 = format!("Parcel Area (Selected): {:.2}", area);
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

fn build_tin(data: &SurfaceData) -> Option<survey_cad::dtm::Tin> {
    if data.vertices.len() < 3 {
        warn!("Failed to build TIN: too few points");
        return None;
    }
    match survey_cad::dtm::Tin::from_points_constrained_with_holes(
        data.vertices.clone(),
        Some(&data.breaklines),
        None,
        &data.holes,
    ) {
        Ok(tin) => Some(tin),
        Err(err) => {
            warn!("Failed to build TIN: {err}");
            None
        }
    }
}

fn handle_build_surface(
    interaction: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<BuildSurfaceButton>),
    >,
    data: Res<SurfaceData>,
    mut tin_res: ResMut<SurfaceTins>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<SurfaceMesh>>,
    mut menu: ResMut<SurfaceMenuState>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        for e in &existing {
            commands.entity(e).despawn_recursive();
        }
        if let Some(tin) = build_tin(&data) {
            let high_mesh = build_surface_mesh(&tin);
            let low_mesh = build_lowres_surface_mesh(&tin);
            let handle = meshes.add(high_mesh);
            let low_handle = meshes.add(low_mesh);
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.0, 1.0, 0.0),
                ..default()
            });
            commands
                .spawn((Mesh3d(handle.clone()), MeshMaterial3d(mat)))
                .insert(SurfaceMesh)
                .insert(LevelOfDetail {
                    high: handle.clone(),
                    low: low_handle.clone(),
                    threshold: 2.0,
                });
            tin_res.0.push(tin);
        }
        if let Some(ent) = menu.entity.take() {
            commands.entity(ent).despawn_recursive();
        }
    }
}

fn build_surface_mesh(tin: &survey_cad::dtm::Tin) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
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
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
fn build_lowres_surface_mesh(tin: &survey_cad::dtm::Tin) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let positions: Vec<[f32; 3]> = tin
        .vertices
        .iter()
        .map(|p| [p.x as f32, p.y as f32, p.z as f32])
        .collect();
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let uvs = vec![[0.0, 0.0]; positions.len()];
    let step = (tin.triangles.len() / 10).max(1);
    let indices: Vec<u32> = tin
        .triangles
        .iter()
        .step_by(step)
        .flat_map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
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

fn handle_show_plan(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<ShowPlanButton>)>,
    mut visible: ResMut<PlanVisible>,
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
    tin_res: Res<SurfaceTins>,
    data: Res<AlignmentData>,
    params: Res<CorridorParams>,
    points: Query<&Transform, With<CadPoint>>,
    mut view: ResMut<SectionView>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        visible.0 = !visible.0;
        view.entities.clear();
        if visible.0 {
            let keep_station = view.station;
            view.sections.clear();
            view.design.clear();
            if let (Some(tin), true) = (tin_res.0.last(), data.points.len() > 1) {
                use survey_cad::alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
                use survey_cad::corridor::{
                    extract_cross_sections, extract_design_cross_sections, Subassembly,
                };
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
                let sub = Subassembly::new(vec![(-params.width, 0.0), (params.width, 0.0)]);
                view.design = extract_design_cross_sections(&align, &[sub], None, params.interval);
            }
            if !view.sections.is_empty() {
                if let Some((idx, _)) = view.sections.iter().enumerate().min_by(|a, b| {
                    (a.1.station - keep_station)
                        .abs()
                        .partial_cmp(&(b.1.station - keep_station).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    view.current = idx;
                    view.station = view.sections[idx].station;
                } else {
                    view.current = 0;
                    view.station = view.sections[0].station;
                }
            }
        } else {
            view.sections.clear();
            view.design.clear();
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
    if let Some(sec) = view.sections.get(view.current) {
        view.station = sec.station;
    }
}

fn handle_section_buttons(
    interactions: Query<(&Interaction, &SectionButton), Changed<Interaction>>,
    mut view: ResMut<SectionView>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed {
            match button.0 {
                SectionControl::ScaleInc => view.scale += 1.0,
                SectionControl::ScaleDec => {
                    view.scale = (view.scale - 1.0).max(0.1);
                }
                SectionControl::ExagInc => view.exaggeration += 0.5,
                SectionControl::ExagDec => {
                    view.exaggeration = (view.exaggeration - 0.5).max(0.1);
                }
                SectionControl::ToggleGround => view.show_ground = !view.show_ground,
                SectionControl::ToggleDesign => view.show_design = !view.show_design,
            }
        }
    }
}

fn handle_view_toggle(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<ViewToggleButton>)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<WorkspaceMode>,
    mut cam2d: Query<&mut Camera, (With<Camera2d>, With<WorkspaceCamera2d>)>,
    mut cam3d: Query<&mut Camera, (With<Camera3d>, With<WorkspaceCamera3d>)>,
) {
    let mut toggle = keys.just_pressed(KeyCode::KeyV);
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        toggle = true;
    }
    if toggle {
        *mode = match *mode {
            WorkspaceMode::TwoD => WorkspaceMode::ThreeD,
            WorkspaceMode::ThreeD => WorkspaceMode::TwoD,
        };
        let active_2d = matches!(*mode, WorkspaceMode::TwoD);
        for mut cam in &mut cam2d {
            cam.is_active = active_2d;
        }
        for mut cam in &mut cam3d {
            cam.is_active = !active_2d;
        }
    }
}

fn handle_grade_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<GradeButton>)>,
    tin_res: Res<SurfaceTins>,
    selected: Res<SelectedPoints>,
    points: Query<&Transform, With<CadPoint>>,
    info: ResMut<GradeInfo>,
    mut spans: Query<&mut TextSpan>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if let (Some(tin), Some(e)) = (tin_res.0.last(), selected.0.first()) {
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

fn handle_select_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<SelectButton>)>,
    mut mode: ResMut<SelectMode>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        mode.0 = !mode.0;
        println!("Select mode {}", if mode.0 { "on" } else { "off" });
    }
}

fn handle_file_menu_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<FileMenuButton>)>,
    mut state: ResMut<FileMenuState>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    theme: Res<ThemeColors>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if let Some(ent) = state.entity.take() {
            commands.entity(ent).despawn_recursive();
        } else {
            let menu = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(5.0),
                        top: Val::Px(60.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(theme.context_bg),
                ))
                .insert(FocusPolicy::Block)
                .with_children(|parent| {
                    parent
                        .spawn(Button)
                        .with_children(|b| {
                            b.spawn((
                                TextLayout::default(),
                                TextFont {
                                    font: asset_server.load("FiraMono-subset.ttf"),
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor::WHITE,
                                Text::new("New"),
                            ));
                        })
                        .insert(NewButton);
                    parent
                        .spawn(Button)
                        .with_children(|b| {
                            b.spawn((
                                TextLayout::default(),
                                TextFont {
                                    font: asset_server.load("FiraMono-subset.ttf"),
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor::WHITE,
                                Text::new("Open"),
                            ));
                        })
                        .insert(OpenButton);
                    parent
                        .spawn(Button)
                        .with_children(|b| {
                            b.spawn((
                                TextLayout::default(),
                                TextFont {
                                    font: asset_server.load("FiraMono-subset.ttf"),
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor::WHITE,
                                Text::new("Save"),
                            ));
                        })
                        .insert(SaveButton);
                })
                .id();
            state.entity = Some(menu);
        }
    }
}

fn handle_cogo_menu_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<CogoMenuButton>)>,
    mut state: ResMut<CogoMenuState>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    theme: Res<ThemeColors>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if let Some(ent) = state.entity.take() {
            commands.entity(ent).despawn_recursive();
        } else {
            let menu = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(160.0),
                        top: Val::Px(60.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(theme.context_bg),
                ))
                .insert(FocusPolicy::Block)
                .with_children(|parent| {
                    for (label, action) in [
                        ("Bearing", CogoAction::Bearing),
                        ("Forward", CogoAction::Forward),
                        ("Intersection", CogoAction::Intersection),
                        ("Level Elev", CogoAction::LevelElevation),
                        ("Vert Angle", CogoAction::VerticalAngle),
                    ] {
                        parent
                            .spawn(Button)
                            .with_children(|b| {
                                b.spawn((
                                    TextLayout::default(),
                                    TextFont {
                                        font: asset_server.load("FiraMono-subset.ttf"),
                                        font_size: 12.0,
                                        ..default()
                                    },
                                    TextColor::WHITE,
                                    Text::new(label),
                                ));
                            })
                            .insert(CogoButton(action));
                    }
                })
                .id();
            state.entity = Some(menu);
        }
    }
}

fn handle_surface_menu_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<SurfaceMenuButton>)>,
    mut state: ResMut<SurfaceMenuState>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    theme: Res<ThemeColors>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if let Some(ent) = state.entity.take() {
            commands.entity(ent).despawn_recursive();
        } else {
            let menu = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(235.0),
                        top: Val::Px(60.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(theme.context_bg),
                ))
                .insert(FocusPolicy::Block)
                .with_children(|parent| {
                    parent
                        .spawn(Button)
                        .with_children(|b| {
                            b.spawn((
                                TextLayout::default(),
                                TextFont {
                                    font: asset_server.load("FiraMono-subset.ttf"),
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor::WHITE,
                                Text::new("Add Points"),
                            ));
                        })
                        .insert(AddSurfaceButton);
                    parent
                        .spawn(Button)
                        .with_children(|b| {
                            b.spawn((
                                TextLayout::default(),
                                TextFont {
                                    font: asset_server.load("FiraMono-subset.ttf"),
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor::WHITE,
                                Text::new("Add Breakline"),
                            ));
                        })
                        .insert(AddBreaklineButton);
                    parent
                        .spawn(Button)
                        .with_children(|b| {
                            b.spawn((
                                TextLayout::default(),
                                TextFont {
                                    font: asset_server.load("FiraMono-subset.ttf"),
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor::WHITE,
                                Text::new("Add Hole"),
                            ));
                        })
                        .insert(AddHoleButton);
                    parent
                        .spawn(Button)
                        .with_children(|b| {
                            b.spawn((
                                TextLayout::default(),
                                TextFont {
                                    font: asset_server.load("FiraMono-subset.ttf"),
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor::WHITE,
                                Text::new("Build Surface"),
                            ));
                        })
                        .insert(BuildSurfaceButton);
                })
                .id();
            state.entity = Some(menu);
        }
    }
}

fn handle_crs_menu_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<CrsMenuButton>)>,
    mut state: ResMut<CrsMenuState>,
    db: Res<CrsDatabase>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    theme: Res<ThemeColors>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if let Some(ent) = state.entity.take() {
            commands.entity(ent).despawn_recursive();
        } else {
            let menu = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(310.0),
                        top: Val::Px(60.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(theme.context_bg),
                ))
                .insert(FocusPolicy::Block)
                .with_children(|parent| {
                    for entry in &db.0 {
                        parent
                            .spawn(Button)
                            .with_children(|b| {
                                b.spawn((
                                    TextLayout::default(),
                                    TextFont {
                                        font: asset_server.load("FiraMono-subset.ttf"),
                                        font_size: 12.0,
                                        ..default()
                                    },
                                    TextColor::WHITE,
                                    Text::new(format!("{} - {}", entry.code, entry.name)),
                                ));
                            })
                            .insert(CrsOption(entry.code.clone()));
                    }
                })
                .id();
            state.entity = Some(menu);
        }
    }
}

fn handle_crs_option_buttons(
    interactions: Query<(&Interaction, &CrsOption), Changed<Interaction>>,
    mut working: ResMut<WorkingCrs>,
    mut state: ResMut<CrsMenuState>,
    mut commands: Commands,
) {
    for (interaction, opt) in &interactions {
        if *interaction == Interaction::Pressed {
            if let Some(code) = opt.0.split(':').nth(1) {
                if let Ok(epsg) = code.parse::<u32>() {
                    working.0 = Crs::from_epsg(epsg);
                    println!("Selected CRS {}", opt.0);
                }
            }
            if let Some(ent) = state.entity.take() {
                commands.entity(ent).despawn_recursive();
            }
        }
    }
}

fn handle_cogo_buttons(
    interactions: Query<(&Interaction, &CogoButton), Changed<Interaction>>,
    selected: Res<SelectedPoints>,
    points: Query<&Transform, With<CadPoint>>,
    mut state: ResMut<CogoMenuState>,
    mut commands: Commands,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed {
            match button.0 {
                CogoAction::Bearing => {
                    if selected.0.len() >= 2 {
                        if let (Ok(a), Ok(b)) =
                            (points.get(selected.0[0]), points.get(selected.0[1]))
                        {
                            let a = Point::new(a.translation.x as f64, a.translation.y as f64);
                            let b = Point::new(b.translation.x as f64, b.translation.y as f64);
                            let bng = survey_cad::surveying::bearing(a, b);
                            println!("Bearing: {:.3} rad", bng);
                        }
                    } else {
                        println!("Select two points for bearing");
                    }
                }
                CogoAction::Forward => {
                    if let Some(&e) = selected.0.first() {
                        if let Ok(t) = points.get(e) {
                            let start = Point::new(t.translation.x as f64, t.translation.y as f64);
                            let p = survey_cad::surveying::forward(start, 0.0, 10.0);
                            println!("Forward point: {:.3},{:.3}", p.x, p.y);
                        }
                    } else {
                        println!("Select a start point for forward");
                    }
                }
                CogoAction::Intersection => {
                    if selected.0.len() >= 4 {
                        if let (Ok(t1), Ok(t2), Ok(t3), Ok(t4)) = (
                            points.get(selected.0[0]),
                            points.get(selected.0[1]),
                            points.get(selected.0[2]),
                            points.get(selected.0[3]),
                        ) {
                            let a = Point::new(t1.translation.x as f64, t1.translation.y as f64);
                            let b = Point::new(t2.translation.x as f64, t2.translation.y as f64);
                            let c = Point::new(t3.translation.x as f64, t3.translation.y as f64);
                            let d = Point::new(t4.translation.x as f64, t4.translation.y as f64);
                            match survey_cad::surveying::line_intersection(a, b, c, d) {
                                Some(p) => println!("Intersection: {:.3},{:.3}", p.x, p.y),
                                None => println!("Lines are parallel"),
                            }
                        }
                    } else {
                        println!("Select four points for intersection");
                    }
                }
                CogoAction::LevelElevation => {
                    println!("Level Elevation tool not implemented");
                }
                CogoAction::VerticalAngle => {
                    println!("Vertical Angle tool not implemented");
                }
            }
            if let Some(ent) = state.entity.take() {
                commands.entity(ent).despawn_recursive();
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
                    Sprite {
                        color: Color::WHITE,
                        custom_size: Some(Vec2::new(a.distance(b), 2.0)),
                        ..default()
                    },
                    Transform::from_translation((a + b) / 2.0)
                        .with_rotation(Quat::from_rotation_z((b - a).y.atan2((b - a).x))),
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
                    Sprite {
                        color: Color::srgb(0.0, 0.0, 1.0),
                        custom_size: Some(Vec2::new(a.distance(b), 2.0)),
                        ..default()
                    },
                    Transform::from_translation(((a + b) / 2.0).extend(0.0))
                        .with_rotation(Quat::from_rotation_z((b - a).y.atan2((b - a).x))),
                    ProfileLine,
                ));
            }
        }
    }
}

fn polyline_point_at(pts: &[Point], dist: f64) -> Option<Point> {
    if pts.len() < 2 {
        return None;
    }
    let mut remaining = dist;
    for pair in pts.windows(2) {
        let seg_len = distance(pair[0], pair[1]);
        if remaining <= seg_len {
            let t = if seg_len == 0.0 {
                0.0
            } else {
                remaining / seg_len
            };
            return Some(Point::new(
                pair[0].x + t * (pair[1].x - pair[0].x),
                pair[0].y + t * (pair[1].y - pair[0].y),
            ));
        }
        remaining -= seg_len;
    }
    pts.last().copied()
}

fn update_plan_labels(
    visible: Res<PlanVisible>,
    data: Res<AlignmentData>,
    points: Query<&Transform, With<CadPoint>>,
    params: Res<CorridorParams>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    existing: Query<Entity, With<PlanLabel>>,
) {
    for e in &existing {
        commands.entity(e).despawn_recursive();
    }
    if !visible.0 {
        return;
    }
    let mut pts = Vec::new();
    for &e in &data.points {
        if let Ok(t) = points.get(e) {
            pts.push(Point::new(t.translation.x as f64, t.translation.y as f64));
        }
    }
    if pts.len() < 2 {
        return;
    }
    let pl = Polyline::new(pts.clone());
    let total = pl.length();
    let mut station = 0.0;
    let interval = params.interval.max(0.1);
    let font = asset_server.load("FiraMono-subset.ttf");
    while station <= total + 0.01 {
        if let Some(p) = polyline_point_at(&pts, station) {
            commands.spawn((
                Text2d::new(format!("{:.0}", station)),
                TextFont {
                    font: font.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor::WHITE,
                Transform::from_translation(Vec3::new(p.x as f32, p.y as f32, 1.0)),
                PlanLabel,
            ));
        }
        station += interval;
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
    if !visible.0 {
        return;
    }
    if view.sections.is_empty() && view.design.is_empty() {
        return;
    }
    let sec_station = if let Some(sec) = view.sections.get(view.current) {
        sec.station
    } else if let Some(sec) = view.design.get(view.current) {
        sec.station
    } else {
        return;
    };
    view.station = sec_station;
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
        align.horizontal.point_at(sec_station),
        align.horizontal.direction_at(sec_station),
        align.vertical.elevation_at(sec_station),
    ) {
        let normal = (-dir.1, dir.0);
        let base = -40.0f32;
        let h_scale = view.scale;
        let v_scale = view.scale * view.exaggeration;

        let draw_section = |section: &survey_cad::corridor::CrossSection,
                            color: Color,
                            cmds: &mut Commands,
                            ents: &mut Vec<Entity>| {
            for pair in section.points.windows(2) {
                let off_a = (pair[0].x - center.x) * normal.0 + (pair[0].y - center.y) * normal.1;
                let off_b = (pair[1].x - center.x) * normal.0 + (pair[1].y - center.y) * normal.1;
                let elev_a = pair[0].z - grade;
                let elev_b = pair[1].z - grade;
                let a = Vec2::new(off_a as f32 * h_scale, base + elev_a as f32 * v_scale);
                let b = Vec2::new(off_b as f32 * h_scale, base + elev_b as f32 * v_scale);
                let ent = cmds
                    .spawn((
                        Sprite {
                            color,
                            custom_size: Some(Vec2::new(a.distance(b).max(1.0), 1.0)),
                            ..default()
                        },
                        Transform::from_translation(((a + b) / 2.0).extend(0.0))
                            .with_rotation(Quat::from_rotation_z((b - a).y.atan2((b - a).x))),
                        SectionLine,
                    ))
                    .id();
                ents.push(ent);
            }
        };

        if view.show_ground {
            if let Some(sec) = view.sections.get(view.current) {
                let clone =
                    survey_cad::corridor::CrossSection::new(sec.station, sec.points.clone());
                draw_section(
                    &clone,
                    Color::srgb(1.0, 1.0, 0.0),
                    &mut commands,
                    &mut view.entities,
                );
            }
        }
        if view.show_design {
            if let Some(sec) = view.design.get(view.current) {
                let clone =
                    survey_cad::corridor::CrossSection::new(sec.station, sec.points.clone());
                draw_section(
                    &clone,
                    Color::srgb(1.0, 0.0, 0.0),
                    &mut commands,
                    &mut view.entities,
                );
            }
        }
    }
    if let Some(id) = view.label {
        if let Ok(mut span) = spans.get_mut(id) {
            span.0 = format!("Station: {:.2}", sec_station);
        }
    }
}

fn handle_new_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<NewButton>)>,
    mut commands: Commands,
    points: Query<Entity, With<CadPoint>>,
    surfaces: Query<Entity, With<SurfaceMesh>>,
    mut alignment: ResMut<AlignmentData>,
    mut surface_data: ResMut<SurfaceData>,
    mut surface_tin: ResMut<SurfaceTins>,
    mut dirty: ResMut<SurfaceDirty>,
) {
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        for e in &points {
            commands.entity(e).despawn_recursive();
        }
        for e in &surfaces {
            commands.entity(e).despawn_recursive();
        }
        alignment.points.clear();
        surface_data.vertices.clear();
        surface_data.breaklines.clear();
        surface_data.holes.clear();
        surface_data.point_map.clear();
        surface_data.set_changed();
        alignment.set_changed();
        surface_tin.0.clear();
        dirty.0 = false;
        println!("New project created");
    }
}

fn handle_open_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<OpenButton>)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut alignment: ResMut<AlignmentData>,
    mut surface_data: ResMut<SurfaceData>,
    mut surface_tin: ResMut<SurfaceTins>,
    mut surface_dirty: ResMut<SurfaceDirty>,
    points: Query<Entity, With<CadPoint>>,
    surfaces: Query<Entity, With<SurfaceMesh>>,
) {
    use survey_cad::io::{landxml, read_points_csv};
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .add_filter("LandXML", &["xml"])
            .add_filter("Shapefile", &["shp"])
            .pick_file()
        {
            let path_str = match path.to_str() {
                Some(s) => s,
                None => {
                    warn!("Selected path could not be read as UTF-8");
                    return;
                }
            };
            for e in &points {
                commands.entity(e).despawn_recursive();
            }
            for e in &surfaces {
                commands.entity(e).despawn_recursive();
            }
            alignment.points.clear();
            surface_data.vertices.clear();
            surface_data.breaklines.clear();
            surface_data.holes.clear();
            surface_data.point_map.clear();
            alignment.set_changed();
            surface_data.set_changed();
            surface_tin.0.clear();
            surface_dirty.0 = false;
            let lower = path_str.to_ascii_lowercase();
            if lower.ends_with(".csv") {
                if let Ok(pts) = read_points_csv(path_str, None, None) {
                    for p in pts {
                        let _ = spawn_point(&mut commands, p);
                    }
                }
            } else if lower.ends_with(".xml") {
                if let Ok(tin) = landxml::read_landxml_surface(path_str) {
                    let mesh = build_surface_mesh(&tin);
                    let handle = meshes.add(mesh);
                    let mat = materials.add(StandardMaterial {
                        base_color: Color::srgb(0.0, 1.0, 0.0),
                        ..default()
                    });
                    commands
                        .spawn((Mesh3d(handle), MeshMaterial3d(mat)))
                        .insert(SurfaceMesh);
                    surface_tin.0.push(tin);
                    surface_data.set_changed();
                } else if let Ok(hal) = landxml::read_landxml_alignment(path_str) {
                    for elem in hal.elements {
                        use survey_cad::alignment::HorizontalElement::*;
                        match elem {
                            Tangent { start, end } => {
                                let a = spawn_point(&mut commands, start);
                                let b = spawn_point(&mut commands, end);
                                alignment.points.push(a);
                                alignment.points.push(b);
                            }
                            Curve { arc } => {
                                let s = Point::new(
                                    arc.center.x + arc.radius * arc.start_angle.cos(),
                                    arc.center.y + arc.radius * arc.start_angle.sin(),
                                );
                                let e = Point::new(
                                    arc.center.x + arc.radius * arc.end_angle.cos(),
                                    arc.center.y + arc.radius * arc.end_angle.sin(),
                                );
                                let a = spawn_point(&mut commands, s);
                                let b = spawn_point(&mut commands, e);
                                alignment.points.push(a);
                                alignment.points.push(b);
                            }
                            Spiral { spiral } => {
                                let a = spawn_point(&mut commands, spiral.start_point());
                                let b = spawn_point(&mut commands, spiral.end_point());
                                alignment.points.push(a);
                                alignment.points.push(b);
                            }
                        }
                        alignment.set_changed();
                    }
                }
            } else if lower.ends_with(".shp") {
                #[cfg(feature = "shapefile")]
                if let Ok((pts, _)) = survey_cad::io::shp::read_points_shp(path_str) {
                    for p in pts {
                        let _ = spawn_point(&mut commands, p);
                    }
                }
                #[cfg(not(feature = "shapefile"))]
                {
                    warn!("Shapefile support not enabled");
                }
            }
        }
    }
}

fn handle_save_button(
    interaction: Query<&Interaction, (Changed<Interaction>, With<Button>, With<SaveButton>)>,
    points: Query<(Entity, &Transform), With<CadPoint>>,
    tin_res: Res<SurfaceTins>,
    alignment: Res<AlignmentData>,
) {
    use survey_cad::io::{landxml, write_points_csv};
    if let Ok(&Interaction::Pressed) = interaction.get_single() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .add_filter("LandXML", &["xml"])
            .add_filter("Shapefile", &["shp"])
            .save_file()
        {
            let path_str = match path.to_str() {
                Some(s) => s,
                None => {
                    warn!("Selected path could not be read as UTF-8");
                    return;
                }
            };
            let lower = path_str.to_ascii_lowercase();
            if lower.ends_with(".csv") {
                let mut pts = Vec::new();
                for (_, t) in &points {
                    pts.push(Point::new(t.translation.x as f64, t.translation.y as f64));
                }
                let _ = write_points_csv(path_str, &pts, None, None);
            } else if lower.ends_with(".xml") {
                if let Some(tin) = tin_res.0.last() {
                    let _ = landxml::write_landxml_surface(path_str, tin);
                } else if alignment.points.len() > 1 {
                    let mut pts = Vec::new();
                    for e in &alignment.points {
                        if let Ok((_, t)) = points.get(*e) {
                            pts.push(Point::new(t.translation.x as f64, t.translation.y as f64));
                        }
                    }
                    let hal = survey_cad::alignment::HorizontalAlignment::new(pts);
                    let _ = landxml::write_landxml_alignment(path_str, &hal);
                }
            } else if lower.ends_with(".shp") {
                #[cfg(feature = "shapefile")]
                {
                    let mut pts = Vec::new();
                    for (_, t) in &points {
                        pts.push(Point::new(t.translation.x as f64, t.translation.y as f64));
                    }
                    let _ = survey_cad::io::shp::write_points_shp(path_str, &pts, None);
                }
                #[cfg(not(feature = "shapefile"))]
                {
                    warn!("Shapefile support not enabled");
                }
            }
        }
    }
}

fn init_ui_scale(windows: Query<&Window>, mut ui_scale: ResMut<UiScale>) {
    ui_scale.0 = windows.single().resolution.scale_factor();
}

fn update_lod_meshes(
    camera_q: Query<&OrthographicProjection, With<Camera2d>>,
    mut meshes: Query<(&mut Mesh3d, &LevelOfDetail)>,
) {
    let scale = camera_q.single().scale;
    for (mut mesh, lod) in &mut meshes {
        let target = if scale > lod.threshold {
            &lod.low
        } else {
            &lod.high
        };
        if mesh.0 != *target {
            mesh.0 = target.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_line_missing_points() {
        let mut app = App::new();
        app.add_systems(Update, create_line);

        let mut input = ButtonInput::<KeyCode>::default();
        input.press(KeyCode::KeyL);
        app.insert_resource(input);
        app.insert_resource(SelectedPoints(vec![
            Entity::from_raw(1),
            Entity::from_raw(2),
        ]));

        app.update();

        let world = app.world_mut();
        assert_eq!(world.query::<&CadLine>().iter(world).count(), 0);
    }

    // Ensure that lines referencing missing points do not cause a panic
    #[test]
    fn update_line_missing_points() {
        let mut world = World::new();
        world.spawn((
            CadLine {
                start: Entity::from_raw(1),
                end: Entity::from_raw(2),
            },
            Transform::default(),
            Sprite::default(),
        ));

        let mut lines = world.query::<&CadLine>();
        for line in lines.iter(&world) {
            if let (Some(_a), Some(_b)) = (
                world.get::<Transform>(line.start),
                world.get::<Transform>(line.end),
            ) {
                // No panic when points are missing
            }
        }
    }
}
