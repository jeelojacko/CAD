mod common;
use image::{DynamicImage, ImageBuffer, Rgba};
use std::sync::Arc;
use truck_meshalgo::prelude::obj;
use truck_platform::*;
use truck_rendimpl::*;
use wgpu::*;

const PICTURE_SIZE: (u32, u32) = (256, 256);

fn bgcheck_shaders(handler: &DeviceHandler) -> PolygonShaders {
    let source = include_str!("shaders/mesh-bindgroup.wgsl");
    let module = Arc::new(
        handler
            .device()
            .create_shader_module(ShaderModuleDescriptor {
                source: ShaderSource::Wgsl(source.into()),
                label: None,
            }),
    );
    PolygonShaders::new(
        Arc::clone(&module),
        "vs_main",
        Arc::clone(&module),
        "nontex_main",
        Arc::clone(&module),
        "tex_main",
    )
}

fn bgcheck_anti_shaders(handler: &DeviceHandler) -> PolygonShaders {
    let source = include_str!("shaders/mesh-bindgroup.wgsl");
    let module = Arc::new(
        handler
            .device()
            .create_shader_module(ShaderModuleDescriptor {
                source: ShaderSource::Wgsl(source.into()),
                label: None,
            }),
    );
    PolygonShaders::new(
        Arc::clone(&module),
        "vs_main",
        Arc::clone(&module),
        "nontex_main_anti",
        Arc::clone(&module),
        "tex_main_anti",
    )
}

const ATTRS_OBJ: &str = "
v -1.0 2.0 -1.0\nv 1.0 2.0 -1.0\nv -1.0 2.0 1.0\nv 1.0 2.0 1.0
vt -1.0 -1.0\nvt 1.0 -1.0\nvt 1.0 1.0\nvt -1.0 1.0
vn -1.0 0.2 -1.0\nvn 1.0 0.2 -1.0\nvn -1.0 0.2 1.0\nvn 1.0 0.2 1.0
";
const TRIS_OBJ: &str = "f 1/1/1 2/2/3 3/4/2\nf 3/4/2 2/2/3 4/3/4\n";
const QUADS_OBJ: &str = "f 1/1/1 2/2/3 4/3/4 3/4/2\n";

fn test_polygons() -> [PolygonMesh; 2] {
    [
        obj::read((ATTRS_OBJ.to_string() + TRIS_OBJ).as_bytes()).unwrap(),
        obj::read((ATTRS_OBJ.to_string() + QUADS_OBJ).as_bytes()).unwrap(),
    ]
}

fn nontex_inst_state() -> PolygonState {
    PolygonState {
        matrix: Matrix4::from_cols(
            [1.0, 2.0, 3.0, 4.0].into(),
            [5.0, 6.0, 7.0, 8.0].into(),
            [9.0, 10.0, 11.0, 12.0].into(),
            [13.0, 14.0, 15.0, 16.0].into(),
        ),
        material: Material {
            albedo: Vector4::new(0.2, 0.4, 0.6, 1.0),
            roughness: 0.31415,
            reflectance: 0.29613,
            ambient_ratio: 0.92,
            background_ratio: 0.32,
            alpha_blend: false,
        },
        texture: None,
        backface_culling: true,
    }
}

fn exec_polygon_bgtest(
    scene: &mut Scene,
    instance: &PolygonInstance,
    answer: &[u8],
    id: usize,
    out_dir: String,
) -> bool {
    let buffer = common::render_one(scene, instance);
    let path = format!("{out_dir}polygon-bgtest-{id}.png");
    common::save_buffer(path, &buffer, PICTURE_SIZE);
    common::same_buffer(answer, &buffer)
}

fn exec_polymesh_nontex_bind_group_test(backend: Backends, out_dir: &str) {
    let out_dir = out_dir.to_string();
    std::fs::create_dir_all(&out_dir).unwrap();
    let instance = wgpu::Instance::new(&InstanceDescriptor {
        backends: backend,
        ..Default::default()
    });
    let handler = common::init_device(&instance);
    let mut scene = Scene::new(
        handler,
        &SceneDescriptor {
            render_texture: RenderTextureConfig {
                canvas_size: PICTURE_SIZE,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let answer = common::nontex_answer_texture(&mut scene);
    let inst_desc = nontex_inst_state();
    test_polygons()
        .iter()
        .enumerate()
        .for_each(move |(i, polygon)| {
            let instance: PolygonInstance = polygon.to_instance(
                scene.device_handler(),
                &bgcheck_shaders(scene.device_handler()),
                &inst_desc,
            );
            assert!(exec_polygon_bgtest(
                &mut scene,
                &instance,
                &answer,
                i,
                out_dir.clone()
            ));
            let instance: PolygonInstance = polygon.to_instance(
                scene.device_handler(),
                &bgcheck_anti_shaders(scene.device_handler()),
                &inst_desc,
            );
            assert!(!exec_polygon_bgtest(
                &mut scene,
                &instance,
                &answer,
                i,
                out_dir.clone()
            ));
        })
}

#[test]
fn polymesh_nontex_bind_group_test() {
    common::os_alt_exec_test(exec_polymesh_nontex_bind_group_test)
}

fn exec_polymesh_tex_bind_group_test(backend: Backends, out_dir: &str) {
    let out_dir = out_dir.to_string();
    std::fs::create_dir_all(&out_dir).unwrap();
    let instance = wgpu::Instance::new(&InstanceDescriptor {
        backends: backend,
        ..Default::default()
    });
    let handler = common::init_device(&instance);
    let mut scene = Scene::new(
        handler,
        &SceneDescriptor {
            render_texture: RenderTextureConfig {
                canvas_size: PICTURE_SIZE,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let buffer = common::random_texture(&mut scene);
    let pngpath = out_dir.clone() + "random-texture.png";
    common::save_buffer(pngpath, &buffer, PICTURE_SIZE);
    let mut state = nontex_inst_state();
    let image_buffer =
        ImageBuffer::<Rgba<_>, _>::from_raw(PICTURE_SIZE.0, PICTURE_SIZE.1, buffer.clone())
            .unwrap();
    let attach = image2texture::image2texture(
        scene.device_handler(),
        &DynamicImage::ImageRgba8(image_buffer),
    );
    state.texture = Some(Arc::new(attach));
    test_polygons()
        .iter()
        .enumerate()
        .for_each(move |(i, polygon)| {
            let instance: PolygonInstance = polygon.to_instance(
                scene.device_handler(),
                &bgcheck_shaders(scene.device_handler()),
                &state,
            );
            assert!(exec_polygon_bgtest(
                &mut scene,
                &instance,
                &buffer,
                i + 3,
                out_dir.clone(),
            ));
            let instance: PolygonInstance = polygon.to_instance(
                scene.device_handler(),
                &bgcheck_anti_shaders(scene.device_handler()),
                &state,
            );
            assert!(!exec_polygon_bgtest(
                &mut scene,
                &instance,
                &buffer,
                i + 3,
                out_dir.clone(),
            ));
        })
}

#[test]
fn polymesh_tex_bind_group_test() { common::os_alt_exec_test(exec_polymesh_tex_bind_group_test) }
