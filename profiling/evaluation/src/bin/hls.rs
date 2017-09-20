//! Evaluate HLS logs and produce runtime accuracy. HLS logs reports each
//! chunk's level. A chunk is a one-second video. This code takes each entry and
//! computes the corresponding accuracy by extracting statistics (true positive,
//! false positive) from it.
//!
//! Example entries in HLS logs:
//! ```ignore
//! 0,5
//! 1,5
//! 2,5
//! ```

extern crate evaluation;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
extern crate csv;

use evaluation::{FrameStat, Profile, VideoConfig, f1, precision, recall};
use std::path::Path;
use std::vec::Vec;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "runtime")]
#[structopt(about = "Evaluate runtime logs and generate accuracy")]
struct Opt {
    /// The path to the stat file that has per-frame stat (true positive, false
    /// positive, false negative).
    #[structopt(short = "s", long = "stat")]
    #[structopt(help = "Path to stat file")]
    stat_path: String,

    /// The path to the runtime log file (that contains frame and level).
    #[structopt(short = "l", long = "log")]
    #[structopt(help = "Path to runtime log")]
    log_path: String,

    /// A profile use to convert from level to configuration.
    #[structopt(short = "p", long = "profile")]
    #[structopt(help = "The path to the profile")]
    profile_path: String,
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    let profile: Profile<VideoConfig> = Profile::new(&opt.profile_path);
    let frame_stats: Vec<FrameStat> = FrameStat::from_csv(&opt.stat_path);
    let logs: Vec<(usize, usize)> = read_log(&opt.log_path);

    // for each log entry, find stat according to the profile
    let per_frame_stat = logs.into_iter()
        .flat_map(|entry| {
            let (second, level) = entry;
            let config = profile.n_th(level);

            // For this `second`, it includes frames in the following range:
            // `second * 30 : (second + 1) * 30`
            ((second * 30)..((second + 1) * 30))
                .map(|frame_num| {
                    let frame = frame_num % 1800;
                    let frame_stat = frame_stats.iter().find(|i| {
                        i.frame_num == frame && i.config == config
                    });
                    match frame_stat {
                        Some(s) => (frame, s.stat),
                        None => {
                            println!("{}, {:?}", frame_num, config);
                            unimplemented!{}
                        }
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    // Split into per second chunks and evaluate accuracy
    for chunk in per_frame_stat.chunks(30) {
        let true_positive = chunk.iter().map(|i| i.1.true_positive).sum::<usize>();
        let false_postive = chunk.iter().map(|i| i.1.false_positive).sum::<usize>();
        let false_negative = chunk.iter().map(|i| i.1.false_negative).sum::<usize>();

        let p = precision(true_positive, false_postive);
        let r = recall(true_positive, false_negative);
        println!("{}", f1(p, r));
    }
}

// Log is a vector of (frame_num, level) pair.
fn read_log<P: AsRef<Path>>(path: P) -> Vec<(usize, usize)> {
    let errmsg = "failed to read log file";
    csv::ReaderBuilder::new()
        .from_path(path)
        .expect(&errmsg)
        .deserialize()
        .map(|r| r.unwrap())
        .collect::<Vec<(usize, usize)>>()
}
