//! Functions and structs for profile.

use super::VideoConfig;
use csv;
use helper;
use rand::{sample, thread_rng};
use rayon::prelude::*;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::Path;
/// Record is each individual rule in a profile.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
struct Record<C> {
    pub bandwidth: f64,
    pub config: C,
    pub accuracy: f64,
}

impl<T: DeserializeOwned + Copy + Debug> Profile<T> {
    /// Creates a new `Profile` instance with a path pointing to the profile
    /// file (CSV). The columns in the file needs to match the config type.
    /// Because this is the loading phase, we bail early (use expect!).
    pub fn new<P: AsRef<Path>>(path: P) -> Profile<T> {
        let errmsg = format!("no profile file {:?}", path.as_ref());
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(path)
            .expect(&errmsg);
        let mut vec = Vec::new();
        for record in rdr.deserialize() {
            let record: Record<T> = record.expect("failed to parse the record");
            let config = Configuration {
                param: record.config,
                bandwidth: record.bandwidth,
                accuracy: record.accuracy,
            };
            vec.push(config);
        }

        Profile { configurations: vec }
    }
}

/// Given a configuration, this function merges bandwidth measure and accuracy
/// measure, returns a vector of (bandwidth, accuracy)
pub fn get_bandwidth_accuracy_for_config(dir: &str, vc: &VideoConfig) -> Vec<(f64, f64)> {
    let bwfile = vc.derive_bw_file(dir);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(&bwfile)
        .unwrap();
    let bw = reader
        .deserialize()
        .map(|record| record.expect("unexpected data format"))
        .collect::<Vec<(usize, f64)>>();

    let accfile = vc.derive_acc_file(dir);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(&accfile)
        .unwrap();
    let acc = reader
        .deserialize()
        .map(|record| record.expect("unexpected data format"))
        .map(|r: (usize, f64)| if r.1.is_nan() { (r.0, 0.0) } else { r })
        .collect::<Vec<(usize, f64)>>();

    bw.iter()
        .zip(acc.iter())
        .map(|elem| ((elem.0).1, (elem.1).1))
        .collect::<Vec<_>>()
}

/// Summarize profile from `dir` to `outdir`. Will produce `profile.csv` and
/// `pareto.csv`.
pub fn summarize_profile(dir: &str, outdir: &str) {
    let configurations = helper::all_configurations();
    let profile = configurations
        .par_iter()
        .map(|&vc| get_bandwidth_accuracy_for_config(&dir, &vc))
        .collect::<Vec<Vec<(f64, f64)>>>();

    let p = profile
        .iter()
        .map(|p| {
            let len = (p.len() - 1) as f64;
            p.iter().take(len as usize).fold((0.0, 0.0), |sum, i| {
                (sum.0 + i.0 / len, sum.1 + i.1 / len)
            })
        })
        .collect::<Vec<_>>();

    let ofile = format!("{}/profile.csv", outdir);
    let mut writer = csv::Writer::from_path(&ofile).expect("failed to open profile.csv");
    let header = ("bandwidth", "width", "skip", "quant", "accuracy");
    writer.serialize(header).expect("failed to write header");
    for (p, vc) in p.iter().zip(configurations.iter()) {
        let entry = (p.0, vc.width, vc.skip, vc.quant, p.1);
        writer.serialize(entry).expect("failed to write to csv");
    }

    let pareto = pareto(&p);
    let mut pareto = pareto
        .iter()
        .map(|&index| {
            let vc = configurations[index];
            let p = p[index];
            (p.0, p.1, vc)
        })
        .collect::<Vec<(f64, f64, VideoConfig)>>();

    // sort by bandwidth demand
    pareto.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    pareto.dedup_by_key(|i| (i.0 * 10.0).round() as usize);

    let ofile = format!("{}/pareto.csv", outdir);
    let mut writer = csv::Writer::from_path(&ofile).expect("failed to open pareto.csv");
    writer.serialize(header).expect("failed to write header");
    for i in pareto {
        let entry = (i.0 * 1_000.0, i.2.width, i.2.skip, i.2.quant, i.1);
        writer.serialize(entry).expect("failed to write to csv");
    }
}

/// Find the pareto set given a list of bandwidth and a list of acc
pub fn pareto(profile: &Vec<(f64, f64)>) -> Vec<usize> {
    let mut p_set = Vec::new();

    for (i, c) in profile.iter().enumerate() {
        if !profile.iter().any(
            |c_prime| c_prime.0 < c.0 && c_prime.1 > c.1,
        )
        {
            p_set.push(i)
        }
    }

    p_set
}

/// A profile is a list of all configuration.
pub struct Profile<T: Copy + Clone> {
    configurations: Vec<Configuration<T>>,
}

impl<T: Copy + Clone> Default for Profile<T> {
    fn default() -> Profile<T> {
        Profile { configurations: Vec::new() }
    }
}

impl<T: Copy + Clone> Profile<T> {
    /// Creats new profile by combining list of parameters and list of
    /// measurements.
    pub fn from(params: &Vec<T>, measures: Vec<(f64, f64)>) -> Self {
        let vec = params
            .iter()
            .zip(measures.iter())
            .map(|(p, m)| {
                Configuration {
                    param: *p,
                    bandwidth: m.0,
                    accuracy: m.1,
                }
            })
            .collect::<Vec<_>>();
        Profile::from_vec(vec)
    }

    /// Creates a profile by a vector of configurations.
    pub fn from_vec(confs: Vec<Configuration<T>>) -> Self {
        Profile { configurations: confs }
    }

    /// Adds new entry to the profile.
    pub fn add(&mut self, t: T, bandwidth: f64, accuracy: f64) {
        self.configurations.push(Configuration {
            param: t,
            bandwidth: bandwidth,
            accuracy: accuracy,
        })
    }

    /// Returns the Pareto-set of the profile.
    pub fn pareto(&self) -> Pareto<T> {
        let mut set = Vec::new();

        for (_i, c) in self.configurations.iter().enumerate() {
            if !self.configurations.iter().any(|c_prime| {
                c_prime.bandwidth < c.bandwidth && c_prime.accuracy > c.accuracy
            })
            {
                set.push(*c);
            }
        }

        set.sort_by(|a, b| {
            a.bandwidth.partial_cmp(&b.bandwidth).unwrap().reverse()
        });
        set.dedup_by_key(|i| (i.bandwidth * 10.0).round() as usize);

        Pareto { set: set }
    }
}

impl<T: PartialEq + Eq + Clone + Copy> Profile<T> {
    /// Find a particular configuration by the parameter.
    pub fn find_by_param(&self, param: &T) -> Configuration<T> {
        *self.configurations
            .iter()
            .find(|c| c.param == *param)
            .unwrap()
    }
}

impl<T: Copy + Clone> Default for Pareto<T> {
    fn default() -> Pareto<T> {
        Pareto { set: Vec::new() }
    }
}

#[derive(Clone, Debug)]
/// The Pareto-optimal set.
pub struct Pareto<T> {
    /// The Pareto-optimal set.
    pub set: Vec<Configuration<T>>,
}

impl<T: Clone + Copy + PartialEq + Eq> Pareto<T> {
    /// Finds a particular item within Pareto-optimal set (may return `None`)
    pub fn find_param(&self, bandwidth: f64) -> Option<T> {
        self.set
            .iter()
            .filter(|i| i.bandwidth < bandwidth)
            .nth(0)
            .map(|c| c.param.clone())
    }

    /// Creates a new subset of Pareto set
    pub fn sample(&self, n: usize) -> Pareto<T> {
        let mut rng = thread_rng();
        let subset = sample(&mut rng, self.set.iter(), n)
            .iter()
            .map(|i| *i.clone())
            .collect::<Vec<_>>();
        Pareto { set: subset }
    }

    /// Finds the difference between the Pareto and another Profile.
    pub fn diff(&self, other: &Profile<T>) -> (f64, f64) {
        self.set
            .iter()
            .map(|c| {
                let b = other.find_by_param(&c.param);
                (b.bandwidth - c.bandwidth, b.accuracy - c.accuracy)
            })
            .map(|(x, y)| (x * x, y * y))  // square distance
            .fold((0.0, 0.0), |sum, i| (sum.0 + i.0, sum.1 + i.1))
    }
}

impl<T: Clone + Copy + ::std::fmt::Debug> ::std::fmt::Display for Pareto<T> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        for c in &self.set {
            write!(
                f,
                "{:7.02}, {:5.02}, {:?}\n",
                c.bandwidth,
                c.accuracy,
                c.param
            )?
        }
        write!(f, "")
    }
}

/// A generic configuration struct that contains parameter, bandwidth and
/// accuracy.
#[derive(Clone, Copy, Debug)]
pub struct Configuration<T> {
    /// Param
    pub param: T,

    /// Bandwidth
    pub bandwidth: f64,

    /// Accuracy
    pub accuracy: f64,
}
