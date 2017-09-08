/// Takes summary directory and produce `profile.csv` and `pareto.csv`.
/// Primarily use for training summarization (i.e. offline profiling).
extern crate evaluation;
use std::env;

fn main() {
    let dir = env::var("DIR").expect("Use DIR=<summary data>");
    let outdir = env::var("OUTPUT_DIR").expect("Use OUTPUT_DIR=<dir>");

    evaluation::summarize_profile(&dir, &outdir);
}
