use libm::exp;
use rand;
use rand::Rng;
use rand_mt::Mt64;
use std::fs::File;
use std::io::{BufWriter, Write};
use rand_ndarray_csv_bfgs::CSV_FILE;

fn main() -> std::io::Result<()> {
    let file = File::create(CSV_FILE)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "\"x\",\"z\",\"y\"")?;
    let mut rng = Mt64::new(20260205_u64);
    for i in 1..=30 {
        let j = i / 13;
        let lambda = exp((0.01 as f64) + 0.1 * (i as f64) + 0.05 * (j as f64));
        writeln!(writer, "{},{},{}", i, j, rpois(&mut rng, lambda))?;
    }
    Ok(())
}

// ポアソン分布を生成する
fn rpois<T: Rng>(rng: &mut T, lambda: f64) -> u64 {
    let mut k: u64 = 0;
    let mut xp = runif(rng);
    while xp >= (-lambda).exp() {
        xp *= runif(rng);
        k += 1;
    }
    k
}

// 非負整数の乱数を生成し、doubleの[0 1)の乱数に変換
fn runif<T: Rng>(rng: &mut T) -> f64 {
    let rn = rng.next_u64();
    let res = 0x3ff0_0000_0000_0000u64 | (rn >> 12);
    f64::from_bits(res) - 1.0
}
