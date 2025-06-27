use pollster::block_on;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use truck_meshalgo::prelude::*;
use truck_modeling::{self as truck, builder};
use truck_modeling::base::{Point2, Point3, Vector4};
use truck_platform::{wgpu, *};
use truck_rendimpl::*;

/// Simple CAD engine based on the Truck crates.
pub struct TruckCadEngine {
    scene: Scene,
    creator: InstanceCreator,
    instances: Vec<PolygonInstance>,
    point_markers: Vec<Option<PolygonInstance>>,
    lines: Vec<Option<WireFrameInstance>>,
    surfaces: Vec<Option<Surface>>,
}

struct Surface {
    instance: PolygonInstance,
    vertices: Vec<truck::base::Point3>,
    triangles: Vec<[usize; 3]>,
}

impl TruckCadEngine {
    fn rebuild_surface_internal(&mut self, surface_id: usize) {
        if let Some(Some(surface)) = self.surfaces.get_mut(surface_id) {
            self.scene.remove_object(&surface.instance);
            let attrs = StandardAttributes {
                positions: surface.vertices.clone(),
                ..Default::default()
            };
            let tri_faces: Vec<[StandardVertex; 3]> = surface
                .triangles
                .iter()
                .map(|t| {
                    [
                        StandardVertex {
                            pos: t[0],
                            uv: None,
                            nor: None,
                        },
                        StandardVertex {
                            pos: t[1],
                            uv: None,
                            nor: None,
                        },
                        StandardVertex {
                            pos: t[2],
                            uv: None,
                            nor: None,
                        },
                    ]
                })
                .collect();
            let faces = Faces::from_tri_and_quad_faces(tri_faces, Vec::new());
            let mesh = PolygonMesh::new(attrs, faces);
            let new_inst = self
                .creator
                .create_instance(&mesh, &PolygonState::default());
            self.scene.add_object(&new_inst);
            surface.instance = new_inst;
        }
    }
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
            point_markers: Vec::new(),
            lines: Vec::new(),
            surfaces: Vec::new(),
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

    /// Add a small cube to visualize a point.
    pub fn add_point_marker(&mut self, p: truck::base::Point3) -> usize {
        let v: truck::topology::Vertex =
            builder::vertex(truck::base::Point3::new(-0.05, -0.05, -0.05));
        let e: truck::topology::Edge = builder::tsweep(&v, truck::base::Vector3::unit_x() * 0.1);
        let f: truck::topology::Face = builder::tsweep(&e, truck::base::Vector3::unit_y() * 0.1);
        let cube: truck::topology::Solid =
            builder::tsweep(&f, truck::base::Vector3::unit_z() * 0.1);
        let state = PolygonState {
            matrix: Matrix4::from_translation(p.to_vec()),
            ..Default::default()
        };
        let mesh = cube.triangulation(0.01).to_polygon();
        let instance = self.creator.create_instance(&mesh, &state);
        self.scene.add_object(&instance);
        self.point_markers.push(Some(instance));
        self.point_markers.len() - 1
    }

    /// Update the location of a point marker.
    pub fn update_point_marker(&mut self, id: usize, p: truck::base::Point3) {
        if let Some(Some(inst)) = self.point_markers.get_mut(id) {
            inst.instance_state_mut().matrix = Matrix4::from_translation(p.to_vec());
        }
    }

    /// Remove a point marker by id.
    pub fn remove_point_marker(&mut self, id: usize) {
        if let Some(slot) = self.point_markers.get_mut(id) {
            if let Some(inst) = slot.take() {
                self.scene.remove_object(&inst);
            }
        }
    }

    /// Add a line as a wireframe instance.
    pub fn add_line(&mut self, a: truck::base::Point3, b: truck::base::Point3) -> usize {
        let poly = PolylineCurve(vec![a, b]);
        let instance = self
            .creator
            .create_instance(&poly, &WireFrameState::default());
        self.scene.add_object(&instance);
        self.lines.push(Some(instance));
        self.lines.len() - 1
    }

    /// Update an existing line.
    pub fn update_line(&mut self, id: usize, a: truck::base::Point3, b: truck::base::Point3) {
        if let Some(Some(inst)) = self.lines.get_mut(id) {
            self.scene.remove_object(inst);
            let poly = PolylineCurve(vec![a, b]);
            let new_inst = self
                .creator
                .create_instance(&poly, &WireFrameState::default());
            self.scene.add_object(&new_inst);
            *inst = new_inst;
        }
    }

    /// Remove a line by id.
    pub fn remove_line(&mut self, id: usize) {
        if let Some(slot) = self.lines.get_mut(id) {
            if let Some(inst) = slot.take() {
                self.scene.remove_object(&inst);
            }
        }
    }

    /// Add a triangulated surface to the scene.
    pub fn add_surface(
        &mut self,
        vertices: &[truck::base::Point3],
        triangles: &[[usize; 3]],
    ) -> usize {
        let attrs = StandardAttributes {
            positions: vertices.to_vec(),
            ..Default::default()
        };
        let tri_faces: Vec<[StandardVertex; 3]> = triangles
            .iter()
            .map(|t| {
                [
                    StandardVertex {
                        pos: t[0],
                        uv: None,
                        nor: None,
                    },
                    StandardVertex {
                        pos: t[1],
                        uv: None,
                        nor: None,
                    },
                    StandardVertex {
                        pos: t[2],
                        uv: None,
                        nor: None,
                    },
                ]
            })
            .collect();
        let faces = Faces::from_tri_and_quad_faces(tri_faces, Vec::new());
        let mesh = PolygonMesh::new(attrs, faces);
        let instance = self
            .creator
            .create_instance(&mesh, &PolygonState::default());
        self.scene.add_object(&instance);
        self.surfaces.push(Some(Surface {
            instance,
            vertices: vertices.to_vec(),
            triangles: triangles.to_vec(),
        }));
        self.surfaces.len() - 1
    }

    /// Update an existing surface.
    pub fn update_surface(
        &mut self,
        id: usize,
        vertices: &[truck::base::Point3],
        triangles: &[[usize; 3]],
    ) {
        if let Some(Some(surface)) = self.surfaces.get_mut(id) {
            self.scene.remove_object(&surface.instance);
            let attrs = StandardAttributes {
                positions: vertices.to_vec(),
                ..Default::default()
            };
            let tri_faces: Vec<[StandardVertex; 3]> = triangles
                .iter()
                .map(|t| {
                    [
                        StandardVertex {
                            pos: t[0],
                            uv: None,
                            nor: None,
                        },
                        StandardVertex {
                            pos: t[1],
                            uv: None,
                            nor: None,
                        },
                        StandardVertex {
                            pos: t[2],
                            uv: None,
                            nor: None,
                        },
                    ]
                })
                .collect();
            let faces = Faces::from_tri_and_quad_faces(tri_faces, Vec::new());
            let mesh = PolygonMesh::new(attrs, faces);
            let new_inst = self
                .creator
                .create_instance(&mesh, &PolygonState::default());
            self.scene.add_object(&new_inst);
            surface.instance = new_inst;
            surface.vertices = vertices.to_vec();
            surface.triangles = triangles.to_vec();
        }
    }

    /// Remove a surface by id.
    pub fn remove_surface(&mut self, id: usize) {
        if let Some(slot) = self.surfaces.get_mut(id) {
            if let Some(surface) = slot.take() {
                self.scene.remove_object(&surface.instance);
            }
        }
    }

    /// Add a vertex to an existing surface and return its index.
    pub fn add_surface_vertex(
        &mut self,
        surface_id: usize,
        vertex: truck::base::Point3,
    ) -> Option<usize> {
        if let Some(Some(surface)) = self.surfaces.get_mut(surface_id) {
            surface.vertices.push(vertex);
            let idx = surface.vertices.len() - 1;
            self.rebuild_surface_internal(surface_id);
            Some(idx)
        } else {
            None
        }
    }

    /// Move an existing vertex of a surface.
    pub fn move_surface_vertex(
        &mut self,
        surface_id: usize,
        vertex_id: usize,
        vertex: truck::base::Point3,
    ) {
        if let Some(Some(surface)) = self.surfaces.get_mut(surface_id) {
            if vertex_id < surface.vertices.len() {
                surface.vertices[vertex_id] = vertex;
                self.rebuild_surface_internal(surface_id);
            }
        }
    }

    /// Delete a vertex from a surface.
    pub fn delete_surface_vertex(&mut self, surface_id: usize, vertex_id: usize) {
        if let Some(Some(surface)) = self.surfaces.get_mut(surface_id) {
            if vertex_id < surface.vertices.len() {
                surface.vertices.remove(vertex_id);
                surface.triangles.retain(|t| !t.contains(&vertex_id));
                for tri in &mut surface.triangles {
                    for v in tri.iter_mut() {
                        if *v > vertex_id {
                            *v -= 1;
                        }
                    }
                }
                self.rebuild_surface_internal(surface_id);
            }
        }
    }

    /// Add a triangle to a surface.
    pub fn add_surface_triangle(&mut self, surface_id: usize, tri: [usize; 3]) {
        if let Some(Some(surface)) = self.surfaces.get_mut(surface_id) {
            if tri.iter().all(|&i| i < surface.vertices.len()) {
                surface.triangles.push(tri);
                self.rebuild_surface_internal(surface_id);
            }
        }
    }

    /// Delete a triangle from a surface.
    pub fn delete_surface_triangle(&mut self, surface_id: usize, tri_id: usize) {
        if let Some(Some(surface)) = self.surfaces.get_mut(surface_id) {
            if tri_id < surface.triangles.len() {
                surface.triangles.remove(tri_id);
                self.rebuild_surface_internal(surface_id);
            }
        }
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

    /// Returns the camera used for rendering.
    pub fn camera(&self) -> &Camera {
        &self.scene.studio_config().camera
    }

    /// Returns the size of the render target.
    pub fn canvas_size(&self) -> (u32, u32) {
        self.scene.descriptor().render_texture.canvas_size
    }

    /// Project a 3D point to screen coordinates (pixels).
    pub fn project_point(&self, p: Point3) -> Option<(f64, f64, f64)> {
        let (w, h) = self.canvas_size();
        if w == 0 || h == 0 {
            return None;
        }
        let aspect = w as f64 / h as f64;
        let ndc = self.camera().projection(aspect).transform_point(p);
        let sx = (ndc.x * 0.5 + 0.5) * w as f64;
        let sy = (1.0 - (ndc.y * 0.5 + 0.5)) * h as f64;
        Some((sx, sy, ndc.z))
    }

    /// Construct a ray from screen coordinates.
    pub fn screen_ray(&self, x: f64, y: f64) -> Ray {
        let (w, h) = self.canvas_size();
        let u = x / w as f64 * 2.0 - 1.0;
        let v = 1.0 - y / h as f64 * 2.0;
        self.camera().ray(Point2::new(u, v))
    }

    /// Get the world position of a point marker.
    pub fn point_marker_position(&self, id: usize) -> Option<Point3> {
        self.point_markers
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|inst| {
                let m = inst.instance_state().matrix;
                Point3::new(m[3].x, m[3].y, m[3].z)
            })
    }

    /// Set the surface color for highlighting.
    pub fn set_surface_color(&mut self, id: usize, color: Vector4) {
        if let Some(Some(surface)) = self.surfaces.get_mut(id) {
            surface.instance.instance_state_mut().material.albedo = color;
            self.scene.update_bind_group(&surface.instance);
        }
    }
}
