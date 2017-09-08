//! This binary takes all profiling results within the `INPUT_DIR` directory and
//! generates per-frame stats: (frame_num, width, skip, quant, true_positive,
//! false_positive, false_negative).

extern crate evaluation;
extern crate rayon;
extern crate csv;

use csv::Writer;
use rayon::prelude::*;
use std::env;

fn main() {
    let dir = env::var("INPUT_DIR").expect("Use INPUT_DIR=<measured dir>");
    let outdir = env::var("OUTPUT_DIR").expect("Use OUTPUT_DIR=<dir>");

    let configurations = evaluation::all_configurations();
    let intermediate = configurations
        .par_iter()
        .map(|&vc| {
            println!("running for {}", vc);
            evaluation::get_frame_stats(&dir, vc)
        })
        .flat_map(|s| s)
        .map(|s| s.to_tuple())
        .collect::<Vec<(usize, usize, usize, usize, usize, usize, usize)>>();

    let outfile = format!("{}/stat.csv", outdir);
    let mut writer = Writer::from_file(outfile).expect("csv open failed");

    for i in intermediate {
        writer.encode(i).expect("failed to write csv");
    }
}
