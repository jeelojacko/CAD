use crate::{prelude::*, *};

/// line
/// # Example
/// ```
/// use truck_geometry::prelude::*;
/// let line = Line(Point2::new(0.0, 0.0), Point2::new(1.0, 1.0));
/// assert_near!(line.subs(0.5), Point2::new(0.5, 0.5));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, SelfSameGeometry)]
pub struct Line<P>(pub P, pub P);

/// unit circle
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, SelfSameGeometry)]
pub struct UnitCircle<P>(std::marker::PhantomData<P>);

/// unit hyperbola
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, SelfSameGeometry)]
pub struct UnitHyperbola<P>(std::marker::PhantomData<P>);

/// parabola whose apex is the origin.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, SelfSameGeometry)]
pub struct UnitParabola<P>(std::marker::PhantomData<P>);

/// plane
/// # Example
/// ```
/// use truck_geometry::prelude::*;
///
/// // arbitrary three points
/// let pt0 = Point3::new(0.0, 1.0, 2.0);
/// let pt1 = Point3::new(1.0, 1.0, 3.0);
/// let pt2 = Point3::new(0.0, 2.0, 3.0);
///
/// // Creates a plane
/// let plane: Plane = Plane::new(pt0, pt1, pt2);
/// // The origin of the plane is pt0.
/// assert_near!(plane.origin(), pt0);
/// // The u-axis of the plane is the vector from pt0 to pt1.
/// assert_near!(plane.u_axis(), pt1 - pt0);
/// // The v-axis of the plane is the vector from pt0 to pt2.
/// assert_near!(plane.v_axis(), pt2 - pt0);
/// // The normal is the normalized u-axis × v-axis
/// assert_near!(plane.normal(), (pt1 - pt0).cross(pt2 - pt0).normalize());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, SelfSameGeometry)]
pub struct Plane {
    o: Point3,
    p: Point3,
    q: Point3,
}

/// sphere
/// # Examples
/// ```
/// use truck_geometry::prelude::*;
/// use std::f64::consts::PI;
///
/// let center = Point3::new(1.0, 2.0, 3.0);
/// let radius = 4.56;
///
/// let sphere = Sphere::new(center, radius);
/// const N: usize = 100;
/// for i in 0..=N {
///     for j in 0..=N {
///         // the parameter u is latitude
///         let u = PI * i as f64 / N as f64;
///         // the parameter v is longitude
///         let v = 2.0 * PI * j as f64 / N as f64;
///
///         // simple relation between a point and its normal.
///         let pt = sphere.subs(u, v);
///         let n = sphere.normal(u, v);
///         assert_near!(pt - center, n * radius);
///
///         // the proof of u is latitude and v is longitude
///         assert!((PI / 2.0 - u) * (pt.z - center.z) >= 0.0);
///         assert!((PI - v) * (pt.y - center.y) >= 0.0);
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, SelfSameGeometry)]
pub struct Sphere {
    center: Point3,
    radius: f64,
}

/// torus
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, SelfSameGeometry)]
pub struct Torus {
    center: Point3,
    large_radius: f64,
    small_radius: f64,
}

mod circle;
mod hyperbola;
mod line;
mod parabola;
mod plane;
mod sphere;
mod torus;

macro_rules! always_true {
    ($ty: tt) => {
        impl<P> PartialEq for $ty<P> {
            fn eq(&self, _: &Self) -> bool { true }
        }
        impl<P> Eq for $ty<P> {}
    };
}

always_true!(UnitCircle);
always_true!(UnitParabola);
always_true!(UnitHyperbola);
