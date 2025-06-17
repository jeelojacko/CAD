mod app;
use app::*;
use std::f64::consts::PI;
use std::sync::Arc;
use truck_meshalgo::prelude::*;
use truck_platform::*;
use truck_rendimpl::*;
use winit::{dpi::*, event::*, keyboard::*};

struct MyApp {
    scene: WindowScene,
    instance0: PolygonInstance,
    instance1: PolygonInstance,
    instance2: WireFrameInstance,
    instance3: WireFrameInstance,
    wireinstance: WireFrameInstance,
    rotate_flag: bool,
    prev_cursor: Vector2,
    render_mode: RenderMode,
}

#[derive(Clone, Copy, Debug)]
enum RenderMode {
    All,
    Surface,
    WireFrame,
    InterferenceOnly,
}

#[async_trait(?Send)]
impl App for MyApp {
    async fn init(window: Arc<winit::window::Window>) -> MyApp {
        let sample_count = 4;
        let matrix = Matrix4::look_at_rh(
            Point3::new(2.0, 2.0, 2.0),
            Point3::origin(),
            Vector3::unit_y(),
        );
        let camera = Camera {
            matrix: matrix.invert().unwrap(),
            method: ProjectionMethod::perspective(Rad(std::f64::consts::PI / 4.0)),
            near_clip: 0.1,
            far_clip: 40.0,
        };
        let scene_desc = WindowSceneDescriptor {
            studio: StudioConfig {
                camera,
                lights: vec![Light {
                    position: Point3::new(2.0, 2.0, 2.0),
                    color: Vector3::new(1.0, 1.0, 1.0),
                    light_type: LightType::Point,
                }],
                ..Default::default()
            },
            backend_buffer: BackendBufferConfig {
                sample_count,
                ..Default::default()
            },
        };
        let mut scene = WindowScene::from_window(window, &scene_desc).await;
        let sphere0 = sphere(Point3::new(0.0, 0.0, 0.7), 1.0, 50, 50);
        let sphere1 = sphere(Point3::new(0.0, 0.0, -0.7), 1.0, 50, 50);
        let intersect = sphere0.extract_interference(&sphere1);
        let creator = scene.instance_creator();
        let instance0 = creator.create_instance(
            &sphere0,
            &PolygonState {
                material: Material {
                    albedo: Vector4::new(1.0, 1.0, 1.0, 0.8),
                    alpha_blend: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let instance1 = creator.create_instance(
            &sphere1,
            &PolygonState {
                material: Material {
                    albedo: Vector4::new(1.0, 1.0, 1.0, 0.8),
                    alpha_blend: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let instance2 = creator.create_instance(&sphere0, &Default::default());
        let instance3 = creator.create_instance(&sphere1, &Default::default());
        let wireinstance = creator.create_instance(
            &intersect,
            &WireFrameState {
                color: Vector4::new(1.0, 0.0, 0.0, 1.0),
                ..Default::default()
            },
        );
        scene.add_object(&instance0);
        scene.add_object(&instance1);
        scene.add_object(&instance2);
        scene.add_object(&instance3);
        scene.add_object(&wireinstance);
        MyApp {
            scene,
            rotate_flag: false,
            prev_cursor: Vector2::zero(),
            instance0,
            instance1,
            instance2,
            instance3,
            wireinstance,
            render_mode: RenderMode::All,
        }
    }
    fn mouse_input(&mut self, state: ElementState, button: MouseButton) -> ControlFlow {
        match button {
            MouseButton::Left => {
                self.rotate_flag = state == ElementState::Pressed;
            }
            MouseButton::Right => {
                let (light, camera) = {
                    let desc = self.scene.studio_config_mut();
                    (&mut desc.lights[0], &desc.camera)
                };
                match light.light_type {
                    LightType::Point => {
                        light.position = camera.position();
                    }
                    LightType::Uniform => {
                        light.position = camera.position();
                        let strength = light.position.to_vec().magnitude();
                        light.position /= strength;
                    }
                }
            }
            _ => {}
        }
        Self::default_control_flow()
    }
    fn mouse_wheel(&mut self, delta: MouseScrollDelta, _: TouchPhase) -> ControlFlow {
        match delta {
            MouseScrollDelta::LineDelta(_, y) => {
                let camera = &mut self.scene.studio_config_mut().camera;
                match &mut camera.method {
                    ProjectionMethod::Parallel { screen_size } => {
                        *screen_size *= 0.9f64.powf(y as f64);
                    }
                    ProjectionMethod::Perspective { .. } => {
                        let trans_vec = camera.eye_direction() * y as f64 * 0.2;
                        camera.matrix = Matrix4::from_translation(trans_vec) * camera.matrix;
                    }
                }
            }
            MouseScrollDelta::PixelDelta(_) => {}
        };
        Self::default_control_flow()
    }

    fn cursor_moved(&mut self, position: PhysicalPosition<f64>) -> ControlFlow {
        let position = Vector2::new(position.x, position.y);
        if self.rotate_flag {
            let matrix = &mut self.scene.studio_config_mut().camera.matrix;
            let position = Vector2::new(position.x, position.y);
            let dir2d = position - self.prev_cursor;
            if dir2d.so_small() {
                return Self::default_control_flow();
            }
            let mut axis = dir2d[1] * matrix[0].truncate();
            axis += dir2d[0] * matrix[1].truncate();
            axis /= axis.magnitude();
            let angle = dir2d.magnitude() * 0.01;
            let mat = Matrix4::from_axis_angle(axis, Rad(angle));
            *matrix = mat.invert().unwrap() * *matrix;
        }
        self.prev_cursor = position;
        Self::default_control_flow()
    }

    fn keyboard_input(&mut self, input: KeyEvent, _: bool) -> ControlFlow {
        if input.state == ElementState::Released {
            return Self::default_control_flow();
        }
        let keycode = match input.physical_key {
            PhysicalKey::Code(keycode) => keycode,
            _ => return Self::default_control_flow(),
        };
        if keycode == KeyCode::Space {
            self.render_mode = match self.render_mode {
                RenderMode::All => RenderMode::Surface,
                RenderMode::Surface => RenderMode::WireFrame,
                RenderMode::WireFrame => RenderMode::InterferenceOnly,
                RenderMode::InterferenceOnly => RenderMode::All,
            };
            self.scene.clear_objects();
            match self.render_mode {
                RenderMode::All => {
                    self.scene.add_object(&self.instance0);
                    self.scene.add_object(&self.instance1);
                    self.scene.add_object(&self.instance2);
                    self.scene.add_object(&self.instance3);
                    self.scene.add_object(&self.wireinstance);
                }
                RenderMode::Surface => {
                    self.scene.add_object(&self.instance0);
                    self.scene.add_object(&self.instance1);
                    self.scene.add_object(&self.wireinstance);
                }
                RenderMode::WireFrame => {
                    self.scene.add_object(&self.instance2);
                    self.scene.add_object(&self.instance3);
                    self.scene.add_object(&self.wireinstance);
                }
                RenderMode::InterferenceOnly => {
                    self.scene.add_object(&self.wireinstance);
                }
            }
        }
        Self::default_control_flow()
    }
    fn render(&mut self) { self.scene.render_frame(); }
}

fn sphere(center: Point3, radius: f64, udiv: usize, vdiv: usize) -> PolygonMesh {
    let positions = (0..udiv)
        .flat_map(move |i| {
            (0..vdiv).map(move |j| {
                let u = 2.0 * PI * i as f64 / udiv as f64;
                let v = PI * j as f64 / (vdiv - 1) as f64;
                center + radius * Vector3::new(u.cos() * v.sin(), u.sin() * v.sin(), v.cos())
            })
        })
        .collect::<Vec<_>>();
    let normals = (0..udiv)
        .flat_map(move |i| {
            (0..vdiv).map(move |j| {
                let u = 2.0 * PI * i as f64 / udiv as f64;
                let v = PI * j as f64 / (vdiv - 1) as f64;
                Vector3::new(u.cos() * v.sin(), u.sin() * v.sin(), v.cos())
            })
        })
        .collect::<Vec<_>>();
    let faces = (0..udiv)
        .flat_map(move |i| {
            (0..vdiv - 1).map(move |j| {
                let a = [
                    i * vdiv + j,
                    i * vdiv + (j + 1) % vdiv,
                    (i + 1) % udiv * vdiv + (j + 1) % vdiv,
                    (i + 1) % udiv * vdiv + j,
                ];
                [
                    (a[0], None, Some(a[0])),
                    (a[1], None, Some(a[1])),
                    (a[2], None, Some(a[2])),
                    (a[3], None, Some(a[3])),
                ]
            })
        })
        .collect();
    PolygonMesh::new(
        StandardAttributes {
            positions,
            normals,
            ..Default::default()
        },
        faces,
    )
}

fn main() { MyApp::run() }
