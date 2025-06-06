// Least squares network adjustment utilities

use crate::geometry::Point;
use super::cogo::bearing;
use nalgebra::{DMatrix, DVector};
use std::collections::{HashMap, HashSet};

/// Supported observation types for a 2D network.
#[derive(Debug, Clone)]
pub enum Observation {
    /// Distance between two points identified by their indices.
    Distance { from: usize, to: usize, value: f64, weight: f64 },
    /// Angle at `at` from the line to `from` to the line to `to` (radians).
    Angle { at: usize, from: usize, to: usize, value: f64, weight: f64 },
}

/// Result of a network adjustment.
#[derive(Debug)]
pub struct AdjustResult {
    pub points: Vec<Point>,
    pub residuals: Vec<f64>,
}

fn bearing_derivatives(p: Point, q: Point) -> (f64, f64, f64, f64) {
    let dx = q.x - p.x;
    let dy = q.y - p.y;
    let denom = dx * dx + dy * dy;
    // derivatives of atan2(dy,dx)
    let d_theta_dx_p = dy / denom;
    let d_theta_dy_p = -dx / denom;
    let d_theta_dx_q = -dy / denom;
    let d_theta_dy_q = dx / denom;
    (d_theta_dx_p, d_theta_dy_p, d_theta_dx_q, d_theta_dy_q)
}

fn angle_partials(points: &[Point], at: usize, from: usize, to: usize) -> (f64, f64, f64, f64, f64, f64) {
    let (dx1_a, dy1_a, dx1_f, dy1_f) = bearing_derivatives(points[at], points[from]);
    let (dx2_a, dy2_a, dx2_t, dy2_t) = bearing_derivatives(points[at], points[to]);
    (
        dx2_a - dx1_a,
        dy2_a - dy1_a,
        -dx1_f,
        -dy1_f,
        dx2_t,
        dy2_t,
    )
}

/// Adjusts a 2D network returning updated coordinates and observation residuals.
pub fn adjust_network(points: &[Point], fixed: &[usize], observations: &[Observation]) -> AdjustResult {
    let fixed_set: HashSet<usize> = fixed.iter().cloned().collect();
    let mut index_map = HashMap::new();
    let mut count = 0usize;
    for i in 0..points.len() {
        if !fixed_set.contains(&i) {
            index_map.insert(i, count);
            count += 2;
        }
    }

    let num_obs = observations.len();
    let mut a = DMatrix::<f64>::zeros(num_obs, count);
    let mut l = DVector::<f64>::zeros(num_obs);
    let mut w = DMatrix::<f64>::zeros(num_obs, num_obs);

    for (row, obs) in observations.iter().enumerate() {
        match *obs {
            Observation::Distance { from, to, value, weight } => {
                let p = points[from];
                let q = points[to];
                let dx = q.x - p.x;
                let dy = q.y - p.y;
                let dist = (dx * dx + dy * dy).sqrt();
                let res = value - dist;
                l[row] = res;
                w[(row, row)] = weight;
                if let Some(&idx) = index_map.get(&from) {
                    a[(row, idx)] = -dx / dist;
                    a[(row, idx + 1)] = -dy / dist;
                }
                if let Some(&idx) = index_map.get(&to) {
                    a[(row, idx)] = dx / dist;
                    a[(row, idx + 1)] = dy / dist;
                }
            }
            Observation::Angle { at, from, to, value, weight } => {
                let b1 = bearing(points[at], points[from]);
                let b2 = bearing(points[at], points[to]);
                let mut angle = b2 - b1;
                while angle > std::f64::consts::PI { angle -= 2.0 * std::f64::consts::PI; }
                while angle < -std::f64::consts::PI { angle += 2.0 * std::f64::consts::PI; }
                let mut res = value - angle;
                while res > std::f64::consts::PI { res -= 2.0 * std::f64::consts::PI; }
                while res < -std::f64::consts::PI { res += 2.0 * std::f64::consts::PI; }
                l[row] = res;
                w[(row, row)] = weight;
                let (da_xa, da_ya, da_xf, da_yf, da_xt, da_yt) = angle_partials(points, at, from, to);
                if let Some(&idx) = index_map.get(&at) {
                    a[(row, idx)] = da_xa;
                    a[(row, idx + 1)] = da_ya;
                }
                if let Some(&idx) = index_map.get(&from) {
                    a[(row, idx)] = da_xf;
                    a[(row, idx + 1)] = da_yf;
                }
                if let Some(&idx) = index_map.get(&to) {
                    a[(row, idx)] = da_xt;
                    a[(row, idx + 1)] = da_yt;
                }
            }
        }
    }

    let at = a.transpose();
    let n = &at * &w * &a;
    let u = &at * &w * &l;
    let delta = match n.clone().lu().solve(&u) {
        Some(d) => d,
        None => return AdjustResult { points: points.to_vec(), residuals: vec![] },
    };
    let v = &a * &delta - &l;

    let mut adj = points.to_vec();
    for (idx, pidx) in index_map.iter() {
        adj[*idx].x += delta[*pidx];
        adj[*idx].y += delta[*pidx + 1];
    }

    AdjustResult {
        points: adj,
        residuals: v.iter().copied().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_distance_network() {
        let pts = vec![Point::new(0.0,0.0), Point::new(100.0,0.0), Point::new(40.0,40.0)];
        let obs = vec![
            Observation::Distance { from:0, to:2, value:(50.0f64.powi(2)+40.0f64.powi(2)).sqrt(), weight:1.0 },
            Observation::Distance { from:1, to:2, value:(50.0f64.powi(2)+40.0f64.powi(2)).sqrt(), weight:1.0 },
        ];
        let res = adjust_network(&pts, &[0,1], &obs);
        let c = res.points[2];
        assert!((c.x - 50.0).abs() < 1e-2);
        assert!((c.y - 40.0).abs() < 1e-2);
        assert!(res.residuals.iter().all(|v| v.abs() < 1e-6));
    }
}
