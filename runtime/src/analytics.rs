use super::evaluation::{self, FrameStat, f1, precision, recall};
use super::profile::Profile;
use super::video::{self, VideoConfig};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::vec::Vec;

#[derive(Clone)]
pub struct VideoAnalytics {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    frame_stats: Vec<FrameStat>,
    profile: Profile<VideoConfig>,

    logs: Vec<(usize, usize)>,
}

/// This is a temporary hack to match two types (despite they have the same
/// fields).
fn match_config(a: video::VideoConfig, b: evaluation::VideoConfig) -> bool {
    a.width == b.width && a.skip == b.skip && a.quant == b.quant
}

impl VideoAnalytics {
    pub fn new<P: AsRef<Path>>(profile: P, stat: P) -> VideoAnalytics {
        let frame_stats: Vec<FrameStat> = FrameStat::from_csv(stat);
        let profile: Profile<VideoConfig> = Profile::new(profile);
        let inner = Inner {
            frame_stats: frame_stats,
            profile: profile,
            logs: Vec::new(),
        };

        VideoAnalytics { inner: Arc::new(Mutex::new(inner)) }
    }

    pub fn add(&mut self, frame_num: usize, level: usize) {
        let mut m = self.inner.lock().unwrap();
        (*m).logs.push((frame_num, level));
    }

    pub fn accuracy(&self) -> f64 {
        let mut m = self.inner.lock().unwrap();
        (*m).accuracy()
    }
}

impl Inner {
    pub fn accuracy(&mut self) -> f64 {
        // for each log entry, find stat according to the profile
        let per_frame_stats = self.logs
            .iter()
            .map(|entry| {
                let (frame, level) = *entry;
                let config = self.profile.n_th(level);

                let frame_stat = self.frame_stats.iter().find(|i| {
                    i.frame_num == frame && match_config(config, i.config)
                });
                frame_stat.expect("failed to find").stat
            })
            .collect::<Vec<_>>();
        let true_positive = per_frame_stats
            .iter()
            .map(|i| i.true_positive)
            .sum::<usize>();
        let false_positive = per_frame_stats
            .iter()
            .map(|i| i.false_positive)
            .sum::<usize>();
        let false_negative = per_frame_stats
            .iter()
            .map(|i| i.false_negative)
            .sum::<usize>();

        let p = precision(true_positive, false_positive);
        let r = recall(true_positive, false_negative);

        self.logs.clear();
        f1(p, r)
    }
}
