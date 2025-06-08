use nalgebra::{DMatrix, DVector};
use survey_cad::surveying::least_squares::{conditional_ls, parametric_ls, redundancy_analysis};

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

#[test]
fn redundancy_analysis_basic() {
    // Simple system with one redundant observation
    let a = DMatrix::from_row_slice(3, 2, &[1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    let l = DVector::from_vec(vec![2.0, 3.0, 5.1]);
    let w = DMatrix::identity(3, 3);
    let res = parametric_ls(&a, &l, &w, None).unwrap();
    let stats = redundancy_analysis(&a, &res.residuals, &w).unwrap();
    let r_sum: f64 = stats.redundancy_numbers.iter().sum();
    assert!((r_sum - 1.0).abs() < 1e-6);
    assert!(stats.studentized_residuals[0] > 0.9);
    assert!(stats.studentized_residuals[2] < -0.9);
    assert!(stats.param_covariance[(0, 0)] > 0.0);
}
