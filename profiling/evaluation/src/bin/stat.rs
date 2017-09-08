//! This binary takes all profiling results within the `INPUT_DIR` directory and
//! generates per-frame stats: (frame_num, width, skip, quant, true_positive,
//! false_positive, false_negative).

extern crate evaluation;
extern crate rayon;
extern crate csv;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use csv::Writer;
use rayon::prelude::*;
use structopt::StructOpt;

fn main() {
    let opt = Opt::from_args();

    let configurations = evaluation::all_configurations();
    let intermediate = configurations
        .par_iter()
        .map(|&vc| {
            println!("running for {}", vc);
            evaluation::get_frame_stats(&opt.input_dir, vc, opt.limit)
        })
        .flat_map(|s| s)
        .map(|s| s.to_tuple())
        .collect::<Vec<(usize, usize, usize, usize, usize, usize, usize)>>();

    let cwd = ".".to_string();
    let outfile = format!("{}/stat.csv", opt.output_dir.unwrap_or(cwd));
    let mut writer = Writer::from_file(outfile).expect("csv open failed");

    for i in intermediate {
        writer.encode(i).expect("failed to write csv");
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "stat")]
#[structopt(about = "Generate per-frame stat from profile output folder.")]
struct Opt {
    /// The folder that contains profiling measurement.
    #[structopt(help = "Input Directory")]
    input_dir: String,

    /// The folder that contains profiling measurement.
    #[structopt(short = "o", long = "out")]
    #[structopt(help = "Output directory, current directory if empty")]
    output_dir: Option<String>,

    /// The limit of frames to process
    #[structopt(short = "l", long = "limit")]
    #[structopt(help = "Number of frames to process")]
    limit: Option<usize>,
}
