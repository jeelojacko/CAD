// General least squares utilities with support for constraint equations and datum stabilization.
// Provides parametric, conditional and free-network adjustment options.

use nalgebra::{DMatrix, DVector, SVD};

/// Result of a least squares computation.
#[derive(Debug)]
pub struct LSResult {
    pub parameters: DVector<f64>,
    pub residuals: DVector<f64>,
}

fn pseudoinverse(m: &DMatrix<f64>, tol: f64) -> DMatrix<f64> {
    let svd = SVD::new(m.clone(), true, true);
    let mut s_inv = svd.singular_values.clone();
    for val in s_inv.iter_mut() {
        if *val > tol {
            *val = 1.0 / *val;
        } else {
            *val = 0.0;
        }
    }
    let u = svd.u.unwrap();
    let vt = svd.v_t.unwrap();
    vt.transpose() * DMatrix::from_diagonal(&s_inv) * u.transpose()
}

/// Performs a parametric least squares adjustment.
///
/// `a` - design matrix relating parameters to observations
/// `l` - misclosure vector (observed - computed)
/// `w` - weight matrix (full support)
/// `constraint` - optional pair `(c, d)` for constraint equations `c * x = d`
pub fn parametric_ls(
    a: &DMatrix<f64>,
    l: &DVector<f64>,
    w: &DMatrix<f64>,
    constraint: Option<(&DMatrix<f64>, &DVector<f64>)>,
) -> Option<LSResult> {
    let at = a.transpose();
    let n = &at * w * a;
    let u = &at * w * l;

    if let Some((c, d)) = constraint {
        let m = n.nrows();
        let k = c.nrows();
        let mut mtx = DMatrix::<f64>::zeros(m + k, m + k);
        mtx.slice_mut((0, 0), (m, m)).copy_from(&n);
        mtx.slice_mut((0, m), (m, k)).copy_from(&c.transpose());
        mtx.slice_mut((m, 0), (k, m)).copy_from(c);
        let mut rhs = DVector::<f64>::zeros(m + k);
        rhs.rows_mut(0, m).copy_from(&u);
        rhs.rows_mut(m, k).copy_from(d);
        let sol = mtx.clone().lu().solve(&rhs).or_else(|| {
            let pinv = pseudoinverse(&mtx, 1e-12);
            Some(pinv * rhs)
        })?;
        let x = sol.rows(0, m).into_owned();
        let v = a * &x - l;
        Some(LSResult { parameters: x, residuals: v })
    } else {
        let sol = n.clone().lu().solve(&u).or_else(|| {
            let pinv = pseudoinverse(&n, 1e-12);
            Some(pinv * u)
        })?;
        let v = a * &sol - l;
        Some(LSResult { parameters: sol, residuals: v })
    }
}

/// Performs a conditional least squares adjustment returning the observation corrections.
///
/// `b` - coefficient matrix of the condition equations
/// `w_vec` - right-hand side of the linearized conditions
/// `p` - weight matrix of the observations
pub fn conditional_ls(
    b: &DMatrix<f64>,
    w_vec: &DVector<f64>,
    p: &DMatrix<f64>,
) -> Option<DVector<f64>> {
    let n = b * p * b.transpose();
    let rhs = w_vec.clone();
    let lambda = n.clone().lu().solve(&rhs).or_else(|| {
        let pinv = pseudoinverse(&n, 1e-12);
        Some(pinv * rhs)
    })?;
    let v = -p * b.transpose() * lambda;
    Some(v)
}

/// Adjusts a free network applying simple centroid constraints for datum stabilization.
pub fn free_network_ls(
    a: &DMatrix<f64>,
    l: &DVector<f64>,
    w: &DMatrix<f64>,
) -> Option<LSResult> {
    let m = a.ncols();
    if m < 2 {
        return parametric_ls(a, l, w, None);
    }
    // constraints: sum dx = 0, sum dy = 0 for translation, and rotation about centroid
    let mut c = DMatrix::<f64>::zeros(3, m);
    for i in 0..(m/2) {
        c[(0, 2*i)] = 1.0;
        c[(1, 2*i + 1)] = 1.0;
        let x = i as f64;
        c[(2, 2*i)] = -x;
        c[(2, 2*i + 1)] = 0.0;
    }
    let d = DVector::<f64>::zeros(3);
    parametric_ls(a, l, w, Some((&c, &d)))
}
