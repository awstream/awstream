/// Process measurement data to generate `bw-XXXX.csv`, `acc-XXXX.csv` and
/// `ts-XXXX.csv`.
extern crate evaluation;
extern crate rayon;

use rayon::prelude::*;
use std::env;

fn main() {
    let dir = env::var("INPUT_DIR").expect("Use INPUT_DIR=<measure data dir>");
    let outdir = env::var("OUTPUT_DIR").expect("Use OUTPUT_DIR=<dir>");

    let configurations = evaluation::all_configurations();
    configurations.par_iter().for_each(|&vc| {
        println!("running for {}", vc);
        evaluation::aggregate_bandwidth(&dir, &outdir, vc, 10);
        evaluation::aggregate_accuracy(&dir, &outdir, vc, 10);
        evaluation::extract_proc_time(&dir, &outdir, vc);
    });
}
