use survey_cad::surveying::least_squares::{parametric_ls, conditional_ls};
use nalgebra::{DMatrix, DVector};

#[test]
fn parametric_basic() {
    // Solve x1 + x2 = 3, x1 - x2 = 1
    let a = DMatrix::from_row_slice(2, 2, &[1.0, 1.0, 1.0, -1.0]);
    let l = DVector::from_vec(vec![3.0, 1.0]);
    let w = DMatrix::identity(2, 2);
    let res = parametric_ls(&a, &l, &w, None).unwrap();
    assert!((res.parameters[0] - 2.0).abs() < 1e-6);
    assert!((res.parameters[1] - 1.0).abs() < 1e-6);
}

#[test]
fn conditional_basic() {
    // Condition: x1 + x2 = 0 with observations x1=1, x2=-2
    let b = DMatrix::from_row_slice(1, 2, &[1.0, 1.0]);
    let wv = DVector::from_vec(vec![1.0 + -2.0]);
    let p = DMatrix::identity(2, 2);
    let v = conditional_ls(&b, &wv, &p).unwrap();
    // Corrections should move to x1=-0.5, x2=-0.5 => v1=-1.5, v2=1.5
    assert!((v[0] + 1.5).abs() < 1e-6);
    assert!((v[1] - 1.5).abs() < 1e-6);
}
