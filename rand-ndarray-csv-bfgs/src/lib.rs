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

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;
    use num_traits::{Float, PrimInt, cast};
    #[test]
    fn llf_works() {
        let test_beta = array![1_f64, -1_f64, 2_f64];
        let test_y = array![1_f64, 0_f64, 1_f64];
        let test_x = array![
            [-0.86_f64, -1.87_f64, -2.87_f64],
            [2.48_f64, -1.08_f64, 1.36_f64],
            [0.38_f64, 1.04_f64, 0.69_f64]
        ];
        let right_llf = -539.8619235072_f64;
        let k = 6;
        let l_v = floor(llf(&test_y, &test_x, &test_beta), k);
        let r_v = floor(right_llf, k);
        println!("{} <> {}", l_v, r_v);
        assert!(l_v <= r_v, "the value of llf is too big!");
        assert!(l_v >= r_v, "the value of llf is too small!");
    }
    #[test]
    fn llfg_works() {
        let test_beta = array![1_f64, -1_f64, 2_f64];
        let test_y = array![1_f64, 0_f64, 1_f64];
        let test_x = array![
            [-0.86_f64, -1.87_f64, -2.87_f64],
            [2.48_f64, -1.08_f64, 1.36_f64],
            [0.38_f64, 1.04_f64, 0.69_f64]
        ];
        let right_llfg = array![
            -1325.0489801423_f64,
            573.5416518934_f64,
            -729.5248097462_f64
        ];
        let r = llfg(&test_y, &test_x, &test_beta);
        let k = 6;
        for i in 0..r.len() {
            let l_v = ceil(r[i], k);
            let r_v = ceil(right_llfg[i], k);
            println!("{} <> {}", l_v, r_v);
            assert!(l_v <= r_v, "the value of llfg[{}] is too big!", i);
            assert!(l_v >= r_v, "the value of llfg[{}] is too small!", i);
        }
    }
    fn ceil<S: PrimInt, T: Float>(x: T, n: S) -> T {
        let ten = T::from(10).unwrap();
        let n_i32: i32 = cast(n).expect("Precision overflowed i32");
        let a = ten.powi(n_i32);
        (x * a).ceil() / a
    }
    fn floor<S: PrimInt, T: Float>(x: T, n: S) -> T {
        let ten = T::from(10).unwrap();
        let n_i32: i32 = cast(n).expect("Precision overflowed i32");
        let a = ten.powi(n_i32);
        (x * a).floor() / a
    }
}
