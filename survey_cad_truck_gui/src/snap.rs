use survey_cad::geometry::{Arc, Line, Point, Polyline};
use survey_cad::io::DxfEntity;
use survey_cad::snap::{snap_point_with_settings, SnapSettings};

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
