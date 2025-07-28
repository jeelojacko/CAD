use criterion::{criterion_group, criterion_main, Criterion};
use truck_cad_engine::TruckCadEngine;
use truck_modeling::base::Point3;

fn build_engine(count: usize) -> TruckCadEngine {
    let mut engine = TruckCadEngine::new(800, 600);
    let vertices: Vec<_> = (0..50)
        .map(|i| Point3::new((i % 10) as f64, (i / 10) as f64, 0.0))
        .collect();
    let triangles: Vec<[usize; 3]> = (0..47).map(|i| [i, i + 1, i + 2]).collect();
    for _ in 0..count {
        engine.add_surface(&vertices, &triangles);
    }
    engine
}

fn bench_render(c: &mut Criterion) {
    let mut engine = build_engine(200);
    c.bench_function("render_no_lod", |b| b.iter(|| engine.render_to_image()));
    engine.enable_lod(30.0);
    c.bench_function("render_lod", |b| b.iter(|| engine.render_to_image()));
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
