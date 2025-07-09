use survey_cad::geometry::{Arc, Line, Point, Polyline};
use survey_cad::io::DxfEntity;
use survey_cad::snap::{snap_point_with_settings, SnapSettings};
use truck_modeling::base::{Point3, Vector3};
use truck_modeling::cgmath::InnerSpace;

pub struct Scene<'a> {
    pub points: &'a [Point],
    pub lines: &'a [(Point, Point)],
    pub polygons: &'a [Vec<Point>],
    pub polylines: &'a [Polyline],
    pub arcs: &'a [Arc],
}

#[derive(Default, Clone, Copy)]
pub struct SnapOptions {
    pub snap_points: bool,
    pub snap_endpoints: bool,
    pub snap_midpoints: bool,
    pub snap_intersections: bool,
    pub snap_nearest: bool,
    pub snap_surfaces: bool,
    pub snap_solids: bool,
}

pub fn resolve_snap(
    target: Point,
    scene: &Scene,
    tol: f64,
    opts: SnapOptions,
) -> Option<Point> {
    let mut ents: Vec<DxfEntity> = Vec::new();
    if opts.snap_points {
        for p in scene.points {
            ents.push(DxfEntity::Point { point: *p, layer: None });
        }
    }
    if opts.snap_endpoints || opts.snap_midpoints || opts.snap_intersections || opts.snap_nearest {
        for (s, e) in scene.lines {
            ents.push(DxfEntity::Line { line: Line::new(*s, *e), layer: None });
        }
        for poly in scene.polygons {
            ents.push(DxfEntity::Polyline { polyline: Polyline::new(poly.clone()), layer: None });
        }
        for pl in scene.polylines {
            ents.push(DxfEntity::Polyline { polyline: pl.clone(), layer: None });
        }
        for arc in scene.arcs {
            ents.push(DxfEntity::Arc { arc: *arc, layer: None });
        }
    }
    let settings = SnapSettings {
        endpoints: opts.snap_points || opts.snap_endpoints,
        midpoints: opts.snap_midpoints,
        intersections: opts.snap_intersections,
        nearest: opts.snap_nearest,
    };
    if ents.is_empty() {
        return None;
    }
    snap_point_with_settings(target, &ents, tol, settings)
}

pub struct Scene3D {
    pub points: Vec<Point3>,
    pub lines: Vec<(Point3, Point3)>,
    pub surface_vertices: Vec<Point3>,
    pub solid_vertices: Vec<Point3>,
}

fn distance3(a: Point3, b: Point3) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

pub fn resolve_snap_3d(target: Point3, scene: &Scene3D, tol: f64, opts: SnapOptions) -> Option<Point3> {
    let mut best = None;
    let mut best_dist = tol;

    if opts.snap_points {
        for p in &scene.points {
            let d = distance3(*p, target);
            if d < best_dist {
                best_dist = d;
                best = Some(*p);
            }
        }
        if opts.snap_surfaces {
            for p in &scene.surface_vertices {
                let d = distance3(*p, target);
                if d < best_dist {
                    best_dist = d;
                    best = Some(*p);
                }
            }
        }
        if opts.snap_solids {
            for p in &scene.solid_vertices {
                let d = distance3(*p, target);
                if d < best_dist {
                    best_dist = d;
                    best = Some(*p);
                }
            }
        }
    }

    if opts.snap_endpoints || opts.snap_midpoints || opts.snap_nearest {
        for (a, b) in &scene.lines {
            if opts.snap_endpoints {
                let da = distance3(*a, target);
                if da < best_dist {
                    best_dist = da;
                    best = Some(*a);
                }
                let db = distance3(*b, target);
                if db < best_dist {
                    best_dist = db;
                    best = Some(*b);
                }
            }
            if opts.snap_midpoints {
                let mid = Point3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0);
                let d = distance3(mid, target);
                if d < best_dist {
                    best_dist = d;
                    best = Some(mid);
                }
            }
            if opts.snap_nearest {
                let ab = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
                let ap = Vector3::new(target.x - a.x, target.y - a.y, target.z - a.z);
                let t = ab.dot(ap) / ab.magnitude2();
                if (0.0..=1.0).contains(&t) {
                    let p = Point3::new(a.x + ab.x * t, a.y + ab.y * t, a.z + ab.z * t);
                    let d = distance3(p, target);
                    if d < best_dist {
                        best_dist = d;
                        best = Some(p);
                    }
                }
            }
        }
    }

    best
}
