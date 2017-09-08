extern crate evaluation;
extern crate rayon;
extern crate itertools;

use evaluation::VideoConfig;
use rayon::prelude::*;
use std::env;

struct Online {
    enable: bool,
    train_duration: usize,
    update_interval: usize,
    trigger: bool,
}

impl Online {
    fn offline() -> Online {
        Online {
            enable: false,
            update_interval: 1,
            train_duration: 0,
            trigger: false,
        }
    }

    fn online() -> Online {
        Online {
            enable: true,
            update_interval: 1,
            train_duration: 3,
            trigger: false,
        }
    }

    fn online_less_time() -> Online {
        Online {
            enable: true,
            update_interval: 3,
            train_duration: 1,
            trigger: false,
        }
    }

    fn trigger() -> Online {
        Online {
            enable: true,
            update_interval: 1,
            train_duration: 3,
            trigger: true,
        }
    }
}

fn eval_online_option(
    configurations: &Vec<VideoConfig>,
    all_bandwidth_accuracy_data: &Vec<Vec<(f64, f64)>>,
    online: Online,
) -> Vec<(f64, f64)> {
    println!("running eval");
    let num_chunk = 24;
    let target_bw = 11.0;

    let mut working_param = evaluation::Configuration {
        param: VideoConfig::new(1280, 0, 20),
        bandwidth: 9.74,
        accuracy: 0.909,
    };

    // empty pareto profile
    let mut sample = evaluation::Pareto::default();

    let mut res = Vec::new();
    for chunk_num in 0..num_chunk {
        // find the index of current working param in configurations
        let idx = configurations
            .iter()
            .position(|c| *c == working_param.param)
            .unwrap();

        // based on idx and chunk_num, we extract the perf
        let perf = all_bandwidth_accuracy_data[idx][chunk_num];
        res.push((perf.0, perf.1));

        // If we have enabled online profiling, we will update working param
        if online.enable && chunk_num > online.train_duration &&
            (chunk_num - online.train_duration).wrapping_rem(online.update_interval) == 0
        {
            let perf_measures = all_bandwidth_accuracy_data
                .iter()
                .map(|p| {
                    let len = online.train_duration as f64;
                    p.iter()
                        .skip(chunk_num - online.train_duration + 1)
                        .take(online.train_duration)
                        .fold((0.0, 0.0), |sum, i| (sum.0 + i.0 / len, sum.1 + i.1 / len))
                })
                .collect::<Vec<_>>();

            let profile = evaluation::Profile::from(&configurations, perf_measures);
            let pareto = profile.pareto();
            let new_param = pareto.find_param(target_bw).expect("no viable param");

            let new_working_param = {
                if !online.trigger {
                    profile.find_by_param(&new_param)
                } else {
                    let diff = sample.diff(&profile);
                    if diff.0 > 5.0 || diff.1 > 0.1 {
                        profile.find_by_param(&new_param)
                    } else {
                        working_param
                    }
                }
            };

            if sample.set.len() == 0 {
                sample = pareto.sample(5);
            }

            if working_param.param != new_working_param.param {
                println!("{}, update {:?}", chunk_num, working_param);

                // if update, we also update sample
                sample = pareto.sample(5);
            }

            working_param = new_working_param;

        }
    }
    res
}

pub fn main() {
    let dir = env::var("DIR").expect("use DIR=<summary data>");

    let configurations = evaluation::all_configurations();
    let all_bandwidth_accuracy_data = configurations
        .par_iter()
        .map(|vc| evaluation::get_bandwidth_accuracy_for_config(&dir, vc))
        .collect::<Vec<Vec<(f64, f64)>>>();

    let offline = eval_online_option(
        &configurations,
        &all_bandwidth_accuracy_data,
        Online::offline(),
    );

    let online = eval_online_option(
        &configurations,
        &all_bandwidth_accuracy_data,
        Online::online(),
    );

    let online_lt = eval_online_option(
        &configurations,
        &all_bandwidth_accuracy_data,
        Online::online_less_time(),
    );

    let trigger = eval_online_option(
        &configurations,
        &all_bandwidth_accuracy_data,
        Online::trigger(),
    );

    for (i, a, b, c, d) in itertools::multizip((0..24, &offline, &online, &online_lt, &trigger)) {
        println!(
            "{}\t{:6.02}\t{:6.02}\t{:6.02}\t{:6.02}\t{:6.02}\t{:6.02}\t{:6.02}\t{:6.02}",
            i,
            a.0,
            a.1,
            b.0,
            b.1,
            c.0,
            c.1,
            d.0,
            d.1
        );
    }
}
