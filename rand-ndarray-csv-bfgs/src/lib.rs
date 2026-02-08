use libm::{exp, lgamma, log};
use ndarray::{Array1, ArrayBase, Data, Ix1, Ix2};

pub const CSV_FILE: &str = "dataset.csv";

pub fn llf<S, T, U>(y: &ArrayBase<S, Ix1>, x: &ArrayBase<T, Ix2>, beta: &ArrayBase<U, Ix1>) -> f64
where
    S: Data<Elem = f64>,
    T: Data<Elem = f64>,
    U: Data<Elem = f64>,
{
    let lp = x.dot(beta); // lp <- X %*% beta
    let mut sum = 0_f64;
    for i in 0..y.len() {
        let lambda = exp(lp[i]);
        sum += y[i] * log(lambda) - lambda - lgamma(y[i] + 1.0_f64);
    }
    sum
}

pub fn llfg<S, T, U>(
    y: &ArrayBase<S, Ix1>,
    x: &ArrayBase<T, Ix2>,
    beta: &ArrayBase<U, Ix1>,
) -> Array1<f64>
where
    S: Data<Elem = f64>,
    T: Data<Elem = f64>,
    U: Data<Elem = f64>,
{
    let lp = x.dot(beta); // lp <- X %*% beta
    let mut dlambda = Array1::<f64>::zeros(beta.len());
    for j in 0..beta.len() {
        for i in 0..lp.len() {
            let lambda = exp(lp[i]);
            dlambda[j] += y[i] * x[[i, j]] - x[[i, j]] * lambda;
        }
        //        dlambda[j] *= -1_f64;
    }
    dlambda
}
