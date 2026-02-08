use csv::ReaderBuilder;
use ndarray::{Array1, Array2, Axis, array, concatenate};
use rand_ndarray_csv_bfgs::{CSV_FILE, llf, llfg};
use std::fs::File;
use wolfe_bfgs::{Bfgs, BfgsSolution};
use std::error::Error;
use std::io::ErrorKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let array = load_csv(CSV_FILE)?;

    // 従属変数をつくる
    let j = 2; // 従属変数の列の位置
    let y = array.column(j);

    // 説明変数をつくる
    let keep_indices: Vec<usize> = (0..array.ncols()).filter(|&k| j != k).collect();
    let x = array.select(Axis(1), &keep_indices);

    // 切片項を足す
    let ones_col = Array2::<f64>::from_elem((x.nrows(), 1), 1.0);
    let x = concatenate![Axis(1), ones_col.view(), x.view()];

    // 目的関数（とそのグラディエント）
    let objf = |beta: &Array1<f64>| -> (f64, Array1<f64>) {
        let f = -llf(&y, &x, beta);
        let g = -llfg(&y, &x, beta);
        (f, g)
    };

    // 初期値
    let beta = array![0_f64, 0_f64, 0_f64];

    // BFGSソルバーを動かす
    let BfgsSolution {
        final_point: beta_min,
        final_value,
        iterations,
        ..
    } = Bfgs::new(beta, objf)
        .with_tolerance(1e-6)
        .with_max_iterations(100)
        .with_fp_tolerances(1e3, 1e2)
        .with_accept_flat_midpoint_once(true)
        .run()
        .expect("BFGS failed to solve");

    println!(
        "Found maximum f([{:.3}, {:.3}, {:.3}]) = {:.4} in {} iterations.",
        beta_min[0], beta_min[1], beta_min[2], -final_value, iterations
    );

    Ok(())
}

fn load_csv(path: &str) -> Result<Array2<f64>, Box<dyn std::error::Error>> {
    let file = File::open(path).map_err(|err| -> Box<dyn Error> {
        if err.kind() == ErrorKind::NotFound {
            // 特定の指示メッセージを返す
            format!("File '{}' not found. Please run 'cargo run --bin gen_ds' first.", path).into()
        } else {
            // それ以外は元のエラーをそのまま Box にして返す
            Box::new(err)
        }
    })?;

    let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);

    // 一旦、1次元のVecとして全データを読み込む
    let mut raw_data = Vec::new();
    let mut rows = 0;

    for result in reader.deserialize() {
        let record: Vec<f64> = result?;
        raw_data.extend(record);
        rows += 1;
    }

    let cols = if rows > 0 { raw_data.len() / rows } else { 0 };

    // Vec から Array2 へ変換
    let matrix = Array2::from_shape_vec((rows, cols), raw_data)?;
    Ok(matrix)
}
