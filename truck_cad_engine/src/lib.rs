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
        Self { scene, creator, instances: Vec::new() }
    }

    /// Add a solid model to the scene.
    pub fn add_solid(&mut self, solid: truck::topology::Solid) {
        let mesh = solid.triangulation(0.01).to_polygon();
        let instance = self.creator.create_instance(&mesh, &PolygonState::default());
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
}
