//! Rotate Objects
//!
//! - Drag the mouse to rotate the camera.
//! - Drag and drop obj files into the window to switch models.
//! - Right-click to move the light to the camera's position.
//! - Enter "P" on the keyboard to switch between parallel projection and perspective projection of the camera.
//! - Enter "L" on the keyboard to switch the point light source/uniform light source of the light.

use std::f64::consts::PI;
use std::io::Read;
use std::sync::Arc;
use truck_meshalgo::prelude::*;
use truck_platform::*;
use truck_rendimpl::*;
use winit::{dpi::*, event::*, keyboard::*};
mod app;
use app::*;

const NUM_OF_OBJECTS: usize = 8;

const TEAPOT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../resources/obj/teapot.obj",
));

struct MyRender {
    scene: WindowScene,
    instances: Vec<PolygonInstance>,
    rotate_flag: bool,
    prev_cursor: Option<Vector2>,
    prev_time: f64,
    path: Option<std::path::PathBuf>,
    light_changed: Option<std::time::Instant>,
    camera_changed: Option<std::time::Instant>,
}

impl MyRender {
    fn create_camera() -> Camera {
        let mat = Matrix4::look_at_rh(
            Point3::new(15.0, 15.0, 15.0),
            Point3::origin(),
            Vector3::unit_y(),
        );
        Camera {
            matrix: mat.invert().unwrap(),
            method: ProjectionMethod::perspective(Rad(PI / 4.0)),
            near_clip: 0.1,
            far_clip: 40.0,
        }
    }

    fn load_obj<R: Read>(&mut self, reader: R) {
        let scene = &mut self.scene;
        scene.clear_objects();
        self.instances.clear();
        let mut mesh = obj::read(reader).unwrap();
        mesh.put_together_same_attrs(TOLERANCE)
            .add_smooth_normals(0.5, false);
        let bdd_box = mesh.bounding_box();
        let (size, center) = (bdd_box.size(), bdd_box.center());
        let original_mesh: PolygonInstance = scene
            .instance_creator()
            .create_instance(&mesh, &Default::default());
        let rad = Rad(2.0 * PI / NUM_OF_OBJECTS as f64);
        let mut mat = Matrix4::from_scale(size / 2.0);
        mat = Matrix4::from_translation(center.to_vec()) * mat;
        mat = mat.invert().unwrap();
        mat = Matrix4::from_translation(Vector3::new(0.0, 0.5, 5.0)) * mat;
        for _ in 0..NUM_OF_OBJECTS {
            let mut instance = original_mesh.clone_instance();
            instance.instance_state_mut().matrix = mat;
            scene.add_object(&instance);
            self.instances.push(instance);
            mat = Matrix4::from_axis_angle(Vector3::unit_y(), rad) * mat;
        }
    }

    fn update_objects(&mut self) {
        let time = self.scene.elapsed().as_secs_f64();
        let delta_time = time - self.prev_time;
        self.prev_time = time;
        let mat0 = Matrix4::from_axis_angle(Vector3::unit_y(), Rad(delta_time));
        for (idx, instance) in self.instances.iter_mut().enumerate() {
            let state = &mut instance.instance_state_mut();
            let (matrix, material) = (&mut state.matrix, &mut state.material);
            let k = (-1_f64).powi(idx as i32) * 5.0;
            let mat1 = Matrix4::from_axis_angle(Vector3::unit_y(), Rad(k * delta_time));
            let x = matrix[3][2];
            let mat = mat0 * *matrix * mat1 * matrix.invert().unwrap();
            *matrix = mat * *matrix;
            let obj_pos = matrix[3].truncate();
            let new_length = 5.0 + time.sin() + (time * 3.0).sin() / 3.0;
            let obj_dir = obj_pos / obj_pos.magnitude();
            let move_vec = obj_dir * (new_length - obj_pos.magnitude());
            let mat = Matrix4::from_translation(move_vec);
            *matrix = mat * *matrix;
            let color = Self::calculate_color(x / 14.0 + 0.5);
            *material = Material {
                albedo: color,
                roughness: (0.5 + (time / 5.0).sin() / 2.0),
                reflectance: 0.04 + 0.96 * (0.5 + (time / 2.0).sin() / 2.0),
                ambient_ratio: 0.02,
                background_ratio: 0.0,
                alpha_blend: false,
            };
            self.scene.update_bind_group(&*instance);
        }
    }

    fn calculate_color(x: f64) -> Vector4 {
        Vector4::new(
            (-25.0 * (x - 0.2) * (x - 0.2)).exp() + (-25.0 * (x - 1.3) * (x - 1.3)).exp(),
            (-25.0 * (x - 0.5) * (x - 0.5)).exp(),
            (-25.0 * (x - 0.8) * (x - 0.8)).exp(),
            1.0,
        )
    }
}

#[async_trait(?Send)]
impl App for MyRender {
    async fn init(window: Arc<winit::window::Window>) -> MyRender {
        let sample_count = 4;
        let scene_desc = WindowSceneDescriptor {
            studio: StudioConfig {
                camera: MyRender::create_camera(),
                lights: vec![Light {
                    position: Point3::new(0.0, 20.0, 0.0),
                    color: Vector3::new(1.0, 1.0, 1.0) * 1.5,
                    light_type: LightType::Point,
                }],
                ..Default::default()
            },
            backend_buffer: BackendBufferConfig {
                sample_count,
                ..Default::default()
            },
        };
        let mut app = MyRender {
            scene: WindowScene::from_window(window, &scene_desc).await,
            instances: Vec::new(),
            rotate_flag: false,
            prev_cursor: None,
            prev_time: 0.0,
            path: None,
            camera_changed: None,
            light_changed: None,
        };
        app.load_obj(TEAPOT_BYTES);
        app
    }

    fn app_title<'a>() -> Option<&'a str> { Some("rotate objects") }

    fn dropped_file(&mut self, path: std::path::PathBuf) -> ControlFlow {
        self.path = Some(path);
        Self::default_control_flow()
    }

    fn mouse_input(&mut self, state: ElementState, button: MouseButton) -> ControlFlow {
        match button {
            MouseButton::Left => {
                self.rotate_flag = state == ElementState::Pressed;
                if !self.rotate_flag {
                    self.prev_cursor = None;
                }
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
                        light.position = Point3::from_vec(camera.position().to_vec().normalize());
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
        if self.rotate_flag {
            let position = Vector2::new(position.x, position.y);
            if let Some(ref prev_position) = self.prev_cursor {
                let camera = &mut self.scene.studio_config_mut().camera;
                let dir2d = position - prev_position;
                if dir2d.so_small() {
                    return Self::default_control_flow();
                }
                let mut axis = dir2d[1] * camera.matrix[0];
                axis += dir2d[0] * camera.matrix[1];
                axis /= axis.magnitude();
                let angle = dir2d.magnitude() * 0.01;
                let mat = Matrix4::from_axis_angle(axis.truncate(), Rad(angle));
                camera.matrix = mat.invert().unwrap() * camera.matrix;
            }
            self.prev_cursor = Some(position);
        }
        Self::default_control_flow()
    }
    fn keyboard_input(&mut self, input: KeyEvent, _: bool) -> ControlFlow {
        let keycode = match input.physical_key {
            PhysicalKey::Code(keycode) => keycode,
            _ => return Self::default_control_flow(),
        };
        match keycode {
            KeyCode::KeyP => {
                if let Some(ref instant) = self.camera_changed {
                    let time = instant.elapsed().as_secs_f64();
                    if time < 0.2 {
                        return Self::default_control_flow();
                    }
                }
                let camera = &mut self.scene.studio_config_mut().camera;
                self.camera_changed = Some(std::time::Instant::now());
                camera.method = match camera.method {
                    ProjectionMethod::Parallel { .. } => {
                        ProjectionMethod::perspective(Rad(PI / 8.0))
                    }
                    ProjectionMethod::Perspective { .. } => ProjectionMethod::parallel(10.0),
                }
            }
            KeyCode::KeyL => {
                if let Some(ref instant) = self.light_changed {
                    let time = instant.elapsed().as_secs_f64();
                    if time < 0.2 {
                        return Self::default_control_flow();
                    }
                }
                self.light_changed = Some(std::time::Instant::now());
                let (light, camera) = {
                    let desc = self.scene.studio_config_mut();
                    (&mut desc.lights[0], &desc.camera)
                };
                *light = match light.light_type {
                    LightType::Point => {
                        let mut vec = camera.position();
                        vec /= vec.to_vec().magnitude();
                        Light {
                            position: vec,
                            color: Vector3::new(1.0, 1.0, 1.0),
                            light_type: LightType::Uniform,
                        }
                    }
                    LightType::Uniform => {
                        let position = camera.position();
                        Light {
                            position,
                            color: Vector3::new(1.0, 1.0, 1.0),
                            light_type: LightType::Point,
                        }
                    }
                };
            }
            _ => {}
        }
        Self::default_control_flow()
    }

    fn render(&mut self) {
        if let Some(path) = self.path.take() {
            let file = std::fs::File::open(path).unwrap();
            self.load_obj(file);
        }
        if self.scene.number_of_objects() != 0 {
            self.update_objects();
        }
        self.scene.render_frame();
    }
}

fn main() { MyRender::run() }
