use truck_geometry::prelude::*;
use truck_stepio::out::*;

fn step_test<T: StepLength>(x: T, ans: &str, length: usize)
where for<'a> StepDisplay<&'a T>: std::fmt::Display {
    let display = StepDisplay::new(&x, 1);
    assert_eq!(&display.to_string(), ans);
    assert_eq!(x.step_length(), length);
    let step = CompleteStepDisplay::new(display, Default::default()).to_string();
    ruststep::parser::parse(&step).unwrap();
}

#[test]
fn geometry() {
    step_test::<Point2>(
        Point2::new(0.0, 1.0),
        "#1 = CARTESIAN_POINT('', (0.0, 1.0));\n",
        1,
    );
    step_test::<Point3>(
        Point3::new(0.0, 1.0, 2.453),
        "#1 = CARTESIAN_POINT('', (0.0, 1.0, 2.453));\n",
        1,
    );
    step_test::<Vector2>(
        Vector2::new(3.0, 4.0),
        "#1 = VECTOR('', #2, 5.0);\n#2 = DIRECTION('', (0.6, 0.8));\n",
        2,
    );
    step_test::<Vector3>(
        Vector3::new(3.0, 4.0, 3.75),
        "#1 = VECTOR('', #2, 6.25);\n#2 = DIRECTION('', (0.48, 0.64, 0.6));\n",
        2,
    );
    step_test::<BSplineCurve<Point2>>(
        BSplineCurve::new(
            KnotVec::bezier_knot(2),
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 1.0),
                Point2::new(2.0, 0.0),
            ],
        ),
            "\
#1 = B_SPLINE_CURVE_WITH_KNOTS('', 2, (#2, #3, #4), .UNSPECIFIED., .U., .U., (3, 3), (0.0, 1.0), .UNSPECIFIED.);
#2 = CARTESIAN_POINT('', (0.0, 0.0));
#3 = CARTESIAN_POINT('', (1.0, 1.0));
#4 = CARTESIAN_POINT('', (2.0, 0.0));\n",
    4,
    );
    step_test::<NurbsCurve<Vector3>>(
        NurbsCurve::new(BSplineCurve::new(
            KnotVec::bezier_knot(2),
            vec![
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(1.0, 1.0, 2.0),
                Vector3::new(2.0, 0.0, 4.0),
            ],
        )),
        "\
#1 = (
    BOUNDED_CURVE()
    B_SPLINE_CURVE(2, (#2, #3, #4), .UNSPECIFIED., .U., .U.)
    B_SPLINE_CURVE_WITH_KNOTS((3, 3), (0.0, 1.0), .UNSPECIFIED.)
    CURVE()
    GEOMETRIC_REPRESENTATION_ITEM()
    RATIONAL_B_SPLINE_CURVE((1.0, 2.0, 4.0))
    REPRESENTATION_ITEM('')
);
#2 = CARTESIAN_POINT('', (0.0, 0.0));
#3 = CARTESIAN_POINT('', (0.5, 0.5));
#4 = CARTESIAN_POINT('', (0.5, 0.0));\n",
        4,
    );
    step_test::<Processor<TrimmedCurve<UnitCircle<Point2>>, Matrix3>>(
        Processor::new(TrimmedCurve::new(UnitCircle::new(), (0.0, 1.0))).transformed(
            Matrix3::from_cols(
                Vector3::new(0.0, 3.0, 0.0),
                Vector3::new(-3.0, 0.0, 0.0),
                Vector3::new(1.0, 2.0, 1.0),
            ),
        ),
        "\
#1 = CIRCLE('', #2, 3.0);
#2 = AXIS2_PLACEMENT_2D('', #3, #4);
#3 = CARTESIAN_POINT('', (1.0, 2.0));
#4 = DIRECTION('', (0.0, 1.0));\n",
        4,
    );
    step_test::<Processor<TrimmedCurve<UnitCircle<Point2>>, Matrix3>>(
        Processor::new(TrimmedCurve::new(UnitCircle::new(), (0.0, 1.0))).transformed(
            Matrix3::from_cols(
                Vector3::new(0.0, 3.0, 0.0),
                Vector3::new(-8.0, 0.0, 0.0),
                Vector3::new(1.0, 2.0, 1.0),
            ),
        ),
        "\
#1 = ELLIPSE('', #2, 3.0, 8.0);
#2 = AXIS2_PLACEMENT_2D('', #3, #4);
#3 = CARTESIAN_POINT('', (1.0, 2.0));
#4 = DIRECTION('', (0.0, 1.0));\n",
        4,
    );
    step_test::<Processor<TrimmedCurve<UnitCircle<Point3>>, Matrix4>>(
        Processor::new(TrimmedCurve::new(UnitCircle::new(), (0.0, 1.0))).transformed(
            Matrix4::from_cols(
                Vector4::new(0.0, 3.0, 0.0, 0.0),
                Vector4::new(-3.0, 0.0, 0.0, 0.0),
                Vector4::new(0.0, 0.0, 3.0, 0.0),
                Vector4::new(3.0, 1.0, 2.0, 1.0),
            ),
        ),
        "\
#1 = CIRCLE('', #2, 3.0);
#2 = AXIS2_PLACEMENT_3D('', #3, #4, #5);
#3 = CARTESIAN_POINT('', (3.0, 1.0, 2.0));
#4 = DIRECTION('', (0.0, 0.0, 1.0));
#5 = DIRECTION('', (0.0, 1.0, 0.0));\n",
        5,
    );
    step_test::<Processor<TrimmedCurve<UnitCircle<Point3>>, Matrix4>>(
        Processor::new(TrimmedCurve::new(UnitCircle::new(), (0.0, 1.0))).transformed(
            Matrix4::from_cols(
                Vector4::new(0.0, 3.0, 0.0, 0.0),
                Vector4::new(-8.0, 0.0, 0.0, 0.0),
                Vector4::new(0.0, 0.0, 1.0, 0.0),
                Vector4::new(3.0, 1.0, 2.0, 1.0),
            ),
        ),
        "\
#1 = ELLIPSE('', #2, 3.0, 8.0);
#2 = AXIS2_PLACEMENT_3D('', #3, #4, #5);
#3 = CARTESIAN_POINT('', (3.0, 1.0, 2.0));
#4 = DIRECTION('', (0.0, 0.0, 1.0));
#5 = DIRECTION('', (0.0, 1.0, 0.0));\n",
        5,
    );
    step_test::<Processor<TrimmedCurve<UnitHyperbola<Point2>>, Matrix3>>(
        Processor::new(TrimmedCurve::new(UnitHyperbola::new(), (0.0, 1.0))).transformed(
            Matrix3::from_cols(
                Vector3::new(0.0, 3.0, 0.0),
                Vector3::new(-8.0, 0.0, 0.0),
                Vector3::new(1.0, 2.0, 1.0),
            ),
        ),
        "\
#1 = HYPERBOLA('', #2, 3.0, 8.0);
#2 = AXIS2_PLACEMENT_2D('', #3, #4);
#3 = CARTESIAN_POINT('', (1.0, 2.0));
#4 = DIRECTION('', (0.0, 1.0));\n",
        4,
    );
    step_test::<Processor<TrimmedCurve<UnitHyperbola<Point3>>, Matrix4>>(
        Processor::new(TrimmedCurve::new(UnitHyperbola::new(), (0.0, 1.0))).transformed(
            Matrix4::from_cols(
                Vector4::new(0.0, 3.0, 0.0, 0.0),
                Vector4::new(-8.0, 0.0, 0.0, 0.0),
                Vector4::new(0.0, 0.0, 1.0, 0.0),
                Vector4::new(3.0, 1.0, 2.0, 1.0),
            ),
        ),
        "\
#1 = HYPERBOLA('', #2, 3.0, 8.0);
#2 = AXIS2_PLACEMENT_3D('', #3, #4, #5);
#3 = CARTESIAN_POINT('', (3.0, 1.0, 2.0));
#4 = DIRECTION('', (0.0, 0.0, 1.0));
#5 = DIRECTION('', (0.0, 1.0, 0.0));\n",
        5,
    );
    step_test::<Processor<TrimmedCurve<UnitParabola<Point2>>, Matrix3>>(
        Processor::new(TrimmedCurve::new(UnitParabola::new(), (0.0, 1.0))).transformed(
            Matrix3::from_cols(
                Vector3::new(0.0, 4.0, 0.0),
                Vector3::new(-2.0, 0.0, 0.0),
                Vector3::new(1.0, 2.0, 1.0),
            ),
        ),
        "\
#1 = PARABOLA('', #2, 1.0);
#2 = AXIS2_PLACEMENT_2D('', #3, #4);
#3 = CARTESIAN_POINT('', (1.0, 2.0));
#4 = DIRECTION('', (0.0, 1.0));\n",
        4,
    );
    step_test::<Processor<TrimmedCurve<UnitParabola<Point3>>, Matrix4>>(
        Processor::new(TrimmedCurve::new(UnitParabola::new(), (0.0, 1.0))).transformed(
            Matrix4::from_cols(
                Vector4::new(0.0, 4.0, 0.0, 0.0),
                Vector4::new(-2.0, 0.0, 0.0, 0.0),
                Vector4::new(0.0, 0.0, 1.0, 0.0),
                Vector4::new(3.0, 1.0, 2.0, 1.0),
            ),
        ),
        "\
#1 = PARABOLA('', #2, 1.0);
#2 = AXIS2_PLACEMENT_3D('', #3, #4, #5);
#3 = CARTESIAN_POINT('', (3.0, 1.0, 2.0));
#4 = DIRECTION('', (0.0, 0.0, 1.0));
#5 = DIRECTION('', (0.0, 1.0, 0.0));\n",
        5,
    );
    step_test::<PCurve<Line<Point2>, Plane>>(
        PCurve::new(
            Line(Point2::new(0.0, 0.0), Point2::new(3.0, 4.0)),
            Plane::new(
                Point3::new(1.0, 2.0, 3.0),
                Point3::new(2.0, 2.0, 3.0),
                Point3::new(1.0, 5.0, 3.0),
            ),
        ),
        "\
#1 = PCURVE('', #2, #7);
#2 = PLANE('', #3);
#3 = AXIS2_PLACEMENT_3D('', #4, #5, #6);
#4 = CARTESIAN_POINT('', (1.0, 2.0, 3.0));
#5 = DIRECTION('', (0.0, 0.0, 1.0));
#6 = DIRECTION('', (1.0, 0.0, 0.0));
#7 = DEFINITIONAL_REPRESENTATION('', (#9), #8);
#8 = (
    GEOMETRIC_REPRESENTATION_CONTEXT(2)
    PARAMETRIC_REPRESENTATION_CONTEXT()
    REPRESENTATION_CONTEXT('2D SPACE', '')
);
#9 = LINE('', #10, #11);
#10 = CARTESIAN_POINT('', (0.0, 0.0));
#11 = VECTOR('', #12, 5.0);
#12 = DIRECTION('', (0.6, 0.8));\n",
        12,
    );
    step_test::<Plane>(
        Plane::new(
            Point3::new(1.0, 2.0, 3.0),
            Point3::new(1.0, 2.0, 4.0),
            Point3::new(2.0, 2.0, 3.0),
        ),
        "\
#1 = PLANE('', #2);
#2 = AXIS2_PLACEMENT_3D('', #3, #4, #5);
#3 = CARTESIAN_POINT('', (1.0, 2.0, 3.0));
#4 = DIRECTION('', (0.0, 1.0, 0.0));
#5 = DIRECTION('', (0.0, 0.0, 1.0));\n",
        5,
    );
    step_test::<Processor<Sphere, Matrix4>>(
        Processor::new(Sphere::new(Point3::new(1.0, 2.0, 3.0), 5.0)).transformed(
            Matrix4::from_cols(
                Vector4::new(0.0, 3.0, 0.0, 0.0),
                Vector4::new(-3.0, 0.0, 0.0, 0.0),
                Vector4::new(0.0, 0.0, 3.0, 0.0),
                Vector4::new(2.0, 1.0, 3.0, 1.0),
            ),
        ),
        "\
#1 = SPHERICAL_SURFACE('', #2, 15.0);
#2 = AXIS2_PLACEMENT_3D('', #3, #4, #5);
#3 = CARTESIAN_POINT('', (3.0, 3.0, 6.0));
#4 = DIRECTION('', (0.0, 0.0, 1.0));
#5 = DIRECTION('', (0.0, 1.0, 0.0));\n",
        5,
    );
    step_test::<Processor<Torus, Matrix4>>(
        Processor::new(Torus::new(Point3::new(1.0, 2.0, 3.0), 5.0, 3.0)).transformed(
            Matrix4::from_cols(
                Vector4::new(0.0, 3.0, 0.0, 0.0),
                Vector4::new(-3.0, 0.0, 0.0, 0.0),
                Vector4::new(0.0, 0.0, 3.0, 0.0),
                Vector4::new(2.0, 1.0, 3.0, 1.0),
            ),
        ),
        "\
#1 = TOROIDAL_SURFACE('', #2, 15.0, 9.0);
#2 = AXIS2_PLACEMENT_3D('', #3, #4, #5);
#3 = CARTESIAN_POINT('', (3.0, 3.0, 6.0));
#4 = DIRECTION('', (0.0, 0.0, 1.0));
#5 = DIRECTION('', (0.0, 1.0, 0.0));\n",
        5,
    );
    step_test::<BSplineSurface<Point2>>(
        BSplineSurface::new(
            (KnotVec::bezier_knot(2), KnotVec::uniform_knot(2, 2)),
            vec![
                vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(0.0, 1.0),
                    Point2::new(0.0, 2.0),
                    Point2::new(0.0, 3.0),
                ],
                vec![
                    Point2::new(1.0, 0.0),
                    Point2::new(1.0, 1.0),
                    Point2::new(1.0, 2.0),
                    Point2::new(1.0, 3.0),
                ],
                vec![
                    Point2::new(2.0, 0.0),
                    Point2::new(2.0, 1.0),
                    Point2::new(2.0, 2.0),
                    Point2::new(2.0, 3.0),
                ],
            ],
        ),
            "\
#1 = B_SPLINE_SURFACE_WITH_KNOTS('', 2, 2, ((#2, #3, #4, #5), (#6, #7, #8, #9), (#10, #11, #12, #13)), .UNSPECIFIED., \
.U., .U., .U., (3, 3), (3, 1, 3), (0.0, 1.0), (0.0, 0.5, 1.0), .UNSPECIFIED.);
#2 = CARTESIAN_POINT('', (0.0, 0.0));
#3 = CARTESIAN_POINT('', (0.0, 1.0));
#4 = CARTESIAN_POINT('', (0.0, 2.0));
#5 = CARTESIAN_POINT('', (0.0, 3.0));
#6 = CARTESIAN_POINT('', (1.0, 0.0));
#7 = CARTESIAN_POINT('', (1.0, 1.0));
#8 = CARTESIAN_POINT('', (1.0, 2.0));
#9 = CARTESIAN_POINT('', (1.0, 3.0));
#10 = CARTESIAN_POINT('', (2.0, 0.0));
#11 = CARTESIAN_POINT('', (2.0, 1.0));
#12 = CARTESIAN_POINT('', (2.0, 2.0));
#13 = CARTESIAN_POINT('', (2.0, 3.0));\n",
    13
    );

    step_test::<NurbsSurface<Vector3>>(
        NurbsSurface::new(BSplineSurface::new(
            (KnotVec::bezier_knot(2), KnotVec::uniform_knot(2, 2)),
            vec![
                vec![
                    Vector3::new(0.0, 0.0, 4.0),
                    Vector3::new(0.0, 1.0, 2.0),
                    Vector3::new(0.0, 2.0, 2.0),
                    Vector3::new(0.0, 3.0, 1.0),
                ],
                vec![
                    Vector3::new(1.0, 0.0, 4.0),
                    Vector3::new(1.0, 1.0, 4.0),
                    Vector3::new(1.0, 2.0, 2.0),
                    Vector3::new(1.0, 3.0, 1.0),
                ],
                vec![
                    Vector3::new(2.0, 0.0, 4.0),
                    Vector3::new(2.0, 1.0, 4.0),
                    Vector3::new(2.0, 2.0, 4.0),
                    Vector3::new(2.0, 3.0, 1.0),
                ],
            ],
        )),
            "\
#1 = (
    BOUNDED_SURFACE()
    B_SPLINE_SURFACE(2, 2, ((#2, #3, #4, #5), (#6, #7, #8, #9), (#10, #11, #12, #13)), .UNSPECIFIED., .U., .U., .U.)
    B_SPLINE_SURFACE_WITH_KNOTS((3, 3), (3, 1, 3), (0.0, 1.0), (0.0, 0.5, 1.0), .UNSPECIFIED.)
    GEOMETRIC_REPRESENTATION_ITEM()
    RATIONAL_B_SPLINE_SURFACE(((4.0, 2.0, 2.0, 1.0), (4.0, 4.0, 2.0, 1.0), (4.0, 4.0, 4.0, 1.0)))
    REPRESENTATION_ITEM('')
    SURFACE()
);
#2 = CARTESIAN_POINT('', (0.0, 0.0));
#3 = CARTESIAN_POINT('', (0.0, 0.5));
#4 = CARTESIAN_POINT('', (0.0, 1.0));
#5 = CARTESIAN_POINT('', (0.0, 3.0));
#6 = CARTESIAN_POINT('', (0.25, 0.0));
#7 = CARTESIAN_POINT('', (0.25, 0.25));
#8 = CARTESIAN_POINT('', (0.5, 1.0));
#9 = CARTESIAN_POINT('', (1.0, 3.0));
#10 = CARTESIAN_POINT('', (0.5, 0.0));
#11 = CARTESIAN_POINT('', (0.5, 0.25));
#12 = CARTESIAN_POINT('', (0.5, 0.5));
#13 = CARTESIAN_POINT('', (2.0, 3.0));\n",
    13
    );
}
