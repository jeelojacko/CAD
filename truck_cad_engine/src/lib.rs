use pollster::block_on;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use truck_meshalgo::prelude::*;
use truck_modeling::{self as truck, builder};
use truck_platform::{wgpu, *};
use truck_rendimpl::*;

/// Simple CAD engine based on the Truck crates.
pub struct TruckCadEngine {
    scene: Scene,
    creator: InstanceCreator,
    instances: Vec<PolygonInstance>,
}

impl TruckCadEngine {
    /// Create a new engine with the given render target size.
    pub fn new(width: u32, height: u32) -> Self {
        let scene_desc = SceneDescriptor {
            studio: StudioConfig::default(),
            backend_buffer: BackendBufferConfig::default(),
            render_texture: RenderTextureConfig {
                canvas_size: (width, height),
                format: wgpu::TextureFormat::Rgba8Unorm,
            },
        };
        let scene = block_on(Scene::from_default_device(&scene_desc));
        let creator = scene.instance_creator();
        Self {
            scene,
            creator,
            instances: Vec::new(),
        }
    }

    /// Add a solid model to the scene.
    pub fn add_solid(&mut self, solid: truck::topology::Solid) {
        let mesh = solid.triangulation(0.01).to_polygon();
        let instance = self
            .creator
            .create_instance(&mesh, &PolygonState::default());
        self.scene.add_object(&instance);
        self.instances.push(instance);
    }

    /// Convenience helper to add a unit cube to the scene.
    pub fn add_unit_cube(&mut self) {
        let v = builder::vertex(truck::base::Point3::new(-0.5, -0.5, -0.5));
        let e = builder::tsweep(&v, truck::base::Vector3::unit_x());
        let f = builder::tsweep(&e, truck::base::Vector3::unit_y());
        let cube = builder::tsweep(&f, truck::base::Vector3::unit_z());
        self.add_solid(cube);
    }

    /// Render the scene into a [`slint::Image`].
    pub fn render_to_image(&mut self) -> Image {
        let bytes = block_on(self.scene.render_to_buffer());
        let (w, h) = self.scene.descriptor().render_texture.canvas_size;
        let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&bytes, w, h);
        Image::from_rgba8_premultiplied(buffer)
    }

    /// Rotate the camera based on pointer movement delta in screen space.
    pub fn rotate_camera(&mut self, dx: f64, dy: f64) {
        let camera = &mut self.scene.studio_config_mut().camera;
        let mut axis = dy * camera.matrix[0].truncate();
        axis += dx * camera.matrix[1].truncate();
        if axis.magnitude() > 0.0 {
            axis /= axis.magnitude();
            let angle = (dx * dx + dy * dy).sqrt() * 0.01;
            let mat = Matrix4::from_axis_angle(axis, Rad(angle));
            camera.matrix = mat.invert().unwrap() * camera.matrix;
        }
    }

    /// Translate the camera parallel to the view plane.
    pub fn pan_camera(&mut self, dx: f64, dy: f64) {
        let camera = &mut self.scene.studio_config_mut().camera;
        let right = camera.matrix[0].truncate();
        let up = camera.matrix[1].truncate();
        let trans = right * (dx * 0.01) - up * (dy * 0.01);
        camera.matrix = Matrix4::from_translation(trans) * camera.matrix;
    }

    /// Zoom the camera along its view direction.
    pub fn zoom_camera(&mut self, delta: f64) {
        let camera = &mut self.scene.studio_config_mut().camera;
        let dir = camera.eye_direction();
        camera.matrix = Matrix4::from_translation(dir * (delta * 0.002)) * camera.matrix;
    }

    /// Resize the render target.
    pub fn resize(&mut self, width: u32, height: u32) {
        let mut desc = self.scene.descriptor_mut();
        desc.render_texture.canvas_size = (width, height);
    }
}
