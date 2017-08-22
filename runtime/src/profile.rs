/// A profile stores the list of <bandwidth, accuracy, configuration>. The
/// simple implementation uses a list and performs binary search for items.
use csv;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::Path;

/// Record is each individual rule in a profile.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Record<C> {
    pub bandwidth: f64,
    pub config: C,
    _accuracy: f64,
}

/// Profile is each individual rule in a profile.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile<C> {
    /// A list of configurations and their respective bandwidth/accuracy info.
    records: Vec<Record<C>>,

    /// The current config (serving as cache)
    current: usize,
}

impl<C: DeserializeOwned + Copy + Debug> Profile<C> {
    /// Creates a new `Profile` instance with a path pointing to the profile
    /// file (CSV). The columns in the file needs to match the config type.
    /// Because this is the loading phase, we bail early (use expect!).
    pub fn new<P: AsRef<Path>>(path: P) -> Profile<C> {
        let errmsg = format!("no profile file {:?}", path.as_ref());
        let mut rdr = csv::Reader::from_path(path).expect(&errmsg);
        let mut vec = Vec::new();
        for record in rdr.deserialize() {
            let record: Record<C> = record.expect("failed to parse the record");
            vec.push(record);
        }

        Profile {
            records: vec,
            current: 0,
        }
    }

    /// Creates a new profile using a vector containing all the records. For
    /// testing purpose.
    pub fn _with_vec(vec: Vec<Record<C>>) -> Profile<C> {
        Profile {
            records: vec,
            current: 0,
        }
    }

    /// Returns the n-th configuration (we will simply do vector indexing).
    pub fn nth(&self, level: usize) -> C {
        self.records[level].config
    }

    /// Returns the initial configuration (we will simply take the first).
    pub fn init_config(&self) -> C {
        self.records
            .first()
            .expect("no configuration in profile")
            .config
    }

    /// Returns the best configuration (we will simply take the last).
    pub fn last_config(&self) -> C {
        self.records
            .last()
            .expect("no configuration in profile")
            .config
    }

    /// Finds the index of the configuration that matches (equal or smaller
    /// than) the provided bandwidth.
    fn get_config_index(&self, bw: f64) -> usize {
        let pos = (&self.records).binary_search_by(|v| {
            v.bandwidth.partial_cmp(&bw).expect(
                "failed to compare bandwidth",
            )
        });
        match pos {
            Ok(i) => i,
            // If error, it could be the first (only 1 profile) or the last
            // (fail to find).
            Err(i) => if i == 0 { 0 } else { i - 1 },
        }
    }

    /// Updates the profile with a configuration that satisfies the provided
    /// bandwidth, i.e., equal or smaller. Returns a tuple of bandwidth and
    /// configuration.
    pub fn update_config(&mut self, bw: f64) -> Option<Record<C>> {
        let new = self.get_config_index(bw);
        if self.current != new {
            self.current = new;
            info!(
                "updating to configuration {:?} (index: {})",
                self.records[self.current],
                self.current
            );
            Some(self.records[self.current])
        } else {
            None
        }
    }

    /// Returns the current configuration.
    pub fn current_config(&self) -> C {
        self.records[self.current].config
    }

    /// Advances to next config. Returns the record if successful; otherwise,
    /// return None (when we cannot advance any more).
    pub fn advance_config(&mut self) -> Option<Record<C>> {
        if self.current < self.records.len() - 1 {
            self.current += 1;
            trace!("advance to configuration {:?}", self.records[self.current]);
            Some(self.records[self.current])
        } else {
            None
        }
    }

    /// Finds out the required rate for next configuration.
    pub fn next_rate(&self) -> Option<f64> {
        if self.current < self.records.len() - 1 {
            Some(self.records[self.current + 1].bandwidth)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Copy, Debug)]
    struct DummyConfig {
        pub v: usize,
    }

    fn create_profile(i: usize) -> Profile<DummyConfig> {
        let mut vec = Vec::new();
        // Populate sample test data
        // 1.0, 2.0, ...
        for i in 0..i {
            let c = DummyConfig { v: i };
            let record = Record {
                bandwidth: i as f64,
                config: c,
                _accuracy: 0.0,
            };
            vec.push(record);
        }
        Profile::_with_vec(vec)
    }

    #[test]
    fn test_profile_simple_get() {
        let mut profile = create_profile(4);
        assert_eq!(profile.init_config().v, 0);
        assert_eq!(profile.last_config().v, 3);
        assert_eq!(profile.current_config().v, 0);
        assert_eq!(profile.update_config(4.0).unwrap().config.v, 3);
        assert_eq!(profile.update_config(1.5).unwrap().config.v, 1);
    }

    #[test]
    fn test_profile_with_one_record() {
        let mut profile = create_profile(1);
        assert_eq!(profile.init_config().v, 0);;
        assert_eq!(profile.last_config().v, 0);
        assert_eq!(profile.current_config().v, 0);
        assert!(profile.update_config(1.5).is_none());
    }
}
