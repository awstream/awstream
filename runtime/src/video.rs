use super::Adapt;
use super::Experiment;
use super::profile::{Profile, SimpleProfile};
use csv;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Deserialize)]
struct Record {
    width: usize,
    skip: usize,
    quant: usize,
    frame: usize,
    bytes: usize,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct VideoConfig {
    pub width: usize,
    pub skip: usize,
    pub quant: usize,
}

impl VideoConfig {
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

pub struct VideoSource {
    map: BTreeMap<(VideoConfig, usize), usize>,
    frame: usize,
    num: usize,
    config: VideoConfig,
    profile: Profile<VideoConfig>,
}

impl VideoSource {
    pub fn new<P>(source: P, profile: P) -> VideoSource
    where
        P: AsRef<Path>,
    {
        let errmsg = format!("no source file {:?}", source.as_ref());
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(source)
            .expect(&errmsg);
        let mut map = BTreeMap::new();
        let mut num = 0;
        for record in rdr.deserialize() {
            let errmsg = "failed to parse the source";
            let record: (VideoConfig, usize, usize) = record.expect(errmsg);
            map.insert((record.0, record.1), record.2);
            num = ::std::cmp::max(num, record.1);
        }

        let p = Profile::new(profile);
        let init = p.init_config();
        VideoSource {
            map: map,
            frame: 1,
            num: num,
            config: init,
            profile: p,
        }
    }

    pub fn profile(&self) -> Profile<VideoConfig> {
        self.profile.clone()
    }

    pub fn next_frame(&mut self) -> usize {
        let frame_size = self.map.get(&(self.config, self.frame)).expect(&format!(
            "Source file corrupted. Failed to find frame size for {}@{}",
            self.config,
            self.frame
        ));
        self.frame += 1;
        if self.frame >= self.num {
            self.frame = 1;
        }
        *frame_size
    }
}

impl Adapt for VideoSource {
    fn adapt(&mut self, bw: f64) {
        match self.profile.adjust_config(bw) {
            Some(c) => self.config = c.config,
            None => {}
        }
    }

    fn current_level(&self) -> usize {
        self.profile.current_level()
    }

    fn dec_degradation(&mut self) {
        match self.profile.advance_config() {
            Some(c) => self.config = c.config,
            None => {}
        }
    }

    fn simple_profile(&self) -> SimpleProfile {
        self.profile.simplify()
    }

    fn period_in_ms(&self) -> u64 {
        33
    }
}

impl Experiment for VideoSource {
    fn next_datum(&mut self) -> usize {
        self.next_frame()
    }
}
