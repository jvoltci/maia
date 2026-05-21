use crate::error::SubstrateError;
use crate::operators::SparseMatrix;

pub fn conjugate_gradient(
    a_mat: &SparseMatrix,
    b: &[f64],
    x0: &[f64],
    tol: f64,
    max_iters: usize,
) -> Result<Vec<f64>, SubstrateError> {
    let n = b.len();
    if a_mat.rows != n {
        return Err(SubstrateError::DimensionMismatch { expected: n, actual: a_mat.rows });
    }
    if a_mat.cols != n {
        return Err(SubstrateError::DimensionMismatch { expected: n, actual: a_mat.cols });
    }
    if x0.len() != n {
        return Err(SubstrateError::DimensionMismatch { expected: n, actual: x0.len() });
    }

    let mut x = x0.to_vec();
    let ax = a_mat.mul_vec(&x)?;
    let mut r = vec![0.0; n];
    for i in 0..n {
        r[i] = b[i] - ax[i];
    }

    // CG stops on ‖r‖ ≤ tol (the true 2-norm of the residual). The β/α
    // updates below still use the squared norm form because the textbook
    // recursions are expressed in `rᵀr` — only the convergence comparison
    // had to be lifted out of the squared domain.
    let mut r_sq_norm = r.iter().map(|&val| val * val).sum::<f64>();
    if r_sq_norm.sqrt() < tol {
        return Ok(x);
    }

    let mut p = r.clone();

    for _ in 0..max_iters {
        let ap = a_mat.mul_vec(&p)?;
        let p_ap = p.iter().zip(ap.iter()).map(|(&pi, &api)| pi * api).sum::<f64>();

        if p_ap.abs() < 1e-14 {
            break;
        }

        let alpha = r_sq_norm / p_ap;

        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }

        let next_r_sq_norm = r.iter().map(|&val| val * val).sum::<f64>();
        if next_r_sq_norm.sqrt() < tol {
            return Ok(x);
        }

        let beta = next_r_sq_norm / r_sq_norm;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }

        r_sq_norm = next_r_sq_norm;
    }

    Ok(x)
}
