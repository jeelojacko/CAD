use rand::{rngs::StdRng, Rng, SeedableRng};
use truck_base::{assert_near, tolerance::Tolerance};
use truck_geotrait::{BoundedCurve, Concat, Cut, ParameterTransform, ParametricCurve};

fn seeded_rng() -> StdRng {
    StdRng::seed_from_u64(0)
}

pub fn parameter_transform_random_test<C>(curve: &C, trials: usize)
where
    C: ParameterTransform,
    C::Point: std::fmt::Debug + Tolerance,
    C::Vector: std::fmt::Debug + Tolerance + std::ops::Mul<f64, Output = C::Vector>,
{
    (0..trials).for_each(|_| exec_parameter_transform_random_test(curve));
}

fn exec_parameter_transform_random_test<C>(curve: &C)
where
    C: ParameterTransform,
    C::Point: std::fmt::Debug + Tolerance,
    C::Vector: std::fmt::Debug + Tolerance + std::ops::Mul<f64, Output = C::Vector>,
{
    let mut rng = seeded_rng();
    let a = rng.random::<f64>() + 0.5;
    let b = rng.random::<f64>() * 2.0;
    let transformed = curve.parameter_transformed(a, b);

    let (t0, t1) = curve.range_tuple();
    assert_near!(transformed.range_tuple().0, t0 * a + b);
    assert_near!(transformed.range_tuple().1, t1 * a + b);
    let p = rng.random::<f64>();
    let t = (1.0 - p) * t0 + p * t1;
    assert_near!(transformed.subs(t * a + b), curve.subs(t));
    assert_near!(transformed.der(t * a + b) * a, curve.der(t));
    assert_near!(transformed.der2(t * a + b) * a * a, curve.der2(t));
    assert_near!(transformed.front(), curve.front());
    assert_near!(transformed.back(), curve.back());
}

pub fn concat_random_test<C0, C1>(curve0: &C0, curve1: &C1, trials: usize)
where
    C0: Concat<C1>,
    C0::Point: std::fmt::Debug + Tolerance,
    C0::Vector: std::fmt::Debug + Tolerance,
    C0::Output: BoundedCurve<Point = C0::Point, Vector = C0::Vector> + std::fmt::Debug,
    C1: BoundedCurve<Point = C0::Point, Vector = C0::Vector>,
{
    (0..trials).for_each(|_| exec_concat_random_test(curve0, curve1));
}

fn exec_concat_random_test<C0, C1>(curve0: &C0, curve1: &C1)
where
    C0: Concat<C1>,
    C0::Point: std::fmt::Debug + Tolerance,
    C0::Vector: std::fmt::Debug + Tolerance,
    C0::Output: BoundedCurve<Point = C0::Point, Vector = C0::Vector> + std::fmt::Debug,
    C1: BoundedCurve<Point = C0::Point, Vector = C0::Vector>,
{
    let mut rng = seeded_rng();
    let concatted = curve0.try_concat(curve1).unwrap();
    let (t0, t1) = curve0.range_tuple();
    let (_, t2) = curve1.range_tuple();
    assert_near!(concatted.range_tuple().0, t0);
    assert_near!(concatted.range_tuple().1, t2);

    let p = rng.random::<f64>();
    let t = t0 * (1.0 - p) + t1 * p;
    assert_near!(concatted.subs(t), curve0.subs(t));
    assert_near!(concatted.der(t), curve0.der(t));
    assert_near!(concatted.der2(t), curve0.der2(t));
    assert_near!(concatted.front(), curve0.front());

    let p = rng.random::<f64>();
    let t = t1 * (1.0 - p) + t2 * p;
    assert_near!(concatted.subs(t), curve1.subs(t));
    assert_near!(concatted.der(t), curve1.der(t));
    assert_near!(concatted.der2(t), curve1.der2(t));
    assert_near!(concatted.back(), curve1.back());
}

pub fn cut_random_test<C>(curve: &C, trials: usize)
where
    C: Cut,
    C::Point: std::fmt::Debug + Tolerance,
    C::Vector: std::fmt::Debug + Tolerance,
{
    (0..trials).for_each(|_| exec_cut_random_test(curve));
}

fn exec_cut_random_test<C>(curve: &C)
where
    C: Cut,
    C::Point: std::fmt::Debug + Tolerance,
    C::Vector: std::fmt::Debug + Tolerance,
{
    let mut rng = seeded_rng();
    let mut part0 = curve.clone();
    let (t0, t1) = curve.range_tuple();
    let p = rng.random::<f64>();
    let t = t0 * (1.0 - p) + t1 * p;
    let part1 = part0.cut(t);
    assert_near!(part0.range_tuple().0, t0);
    assert_near!(part0.range_tuple().1, t);
    assert_near!(part1.range_tuple().0, t);
    assert_near!(part1.range_tuple().1, t1);

    let p = rng.random::<f64>();
    let s = t0 * (1.0 - p) + t * p;
    assert_near!(part0.subs(s), curve.subs(s));
    assert_near!(part0.der(s), curve.der(s));
    assert_near!(part0.der2(s), curve.der2(s));
    assert_near!(part0.front(), curve.front());
    assert_near!(part0.back(), curve.subs(t));

    let p = rng.random::<f64>();
    let s = t * (1.0 - p) + t1 * p;
    assert_near!(part1.subs(s), curve.subs(s));
    assert_near!(part1.der(s), curve.der(s));
    assert_near!(part1.der2(s), curve.der2(s));
    assert_near!(part1.front(), curve.subs(t));
    assert_near!(part1.back(), curve.back());
}
