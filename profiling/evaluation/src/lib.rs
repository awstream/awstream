//! Library of functions and structs to help with AWStream evaluation.

#![deny(missing_docs)]

extern crate csv;
#[macro_use]
extern crate itertools;
extern crate rand;
#[macro_use]
extern crate log;
extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;

mod acc;
pub use acc::aggregate_accuracy;
pub use acc::extract_proc_time;
pub use acc::get_frame_stats;

mod helper;
pub use helper::all_configurations;

mod profile;
pub use profile::Configuration;
pub use profile::Pareto;
pub use profile::Profile;
pub use profile::get_bandwidth_accuracy_for_config;
pub use profile::summarize_profile;

mod bw;
pub use bw::aggregate_bandwidth;

use std::fs::File;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Video configuration consists of width, skip and quantization
pub struct VideoConfig {
    /// frame width
    pub width: usize,

    /// skips per second (translate to fps)
    pub skip: usize,

    /// quantization level used in h264 encoding
    pub quant: usize,
}

impl VideoConfig {
    /// Creates a new `VideoConfig`
    pub fn new(w: usize, s: usize, q: usize) -> Self {
        VideoConfig {
            width: w,
            skip: s,
            quant: q,
        }
    }
}

impl ::std::fmt::Display for VideoConfig {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}x{}x{}", self.width, self.skip, self.quant)
    }
}

#[inline]
fn gt_file(dir: &str) -> File {
    File::open(format!("{}/groundtruth.csv", dir)).expect("no groundtruth file")
}

impl VideoConfig {
    /// Gets the filename of accuracy file.
    pub fn derive_acc_file(&self, dir: &str) -> String {
        format!(
            "{}/acc-{}x{}x{}.csv",
            dir,
            self.width,
            self.skip,
            self.quant
        )
    }

    /// Gets the filename of timestamp file.
    pub fn derive_ts_file(&self, dir: &str) -> String {
        format!("{}/ts-{}x{}x{}.csv", dir, self.width, self.skip, self.quant)
    }

    /// Gets the filename of bandwidth file.
    pub fn derive_bw_file(&self, dir: &str) -> String {
        format!("{}/bw-{}x{}x{}.csv", dir, self.width, self.skip, self.quant)
    }

    /// Opens accuracy file.
    pub fn open_acc_file(&self, dir: &str) -> File {
        let filename = self.derive_acc_file(dir);
        File::open(&filename).expect(&format!("no input file: {}", filename))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_pareto() {
        let mut profile = Profile::default();
        profile.add(1, 1.0, 1.1);
        profile.add(2, 2.0, 1.0);
        profile.add(3, 3.0, 2.0);
        let pareto = profile.pareto();

        let set = pareto.set.iter().map(|i| i.param).collect::<Vec<usize>>();
        assert_eq!(vec![3, 1], set);
    }
}
