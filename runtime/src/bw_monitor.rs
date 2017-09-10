use std::sync::{Arc, Mutex};
use std::vec::Vec;

#[derive(Clone)]
pub struct BwMonitor {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner {
    sample: usize,
    rate: f64,
}

impl BwMonitor {
    pub fn new() -> BwMonitor {
        let inner = Inner {
            sample: 0,
            rate: 0.0,
        };
        BwMonitor { inner: Arc::new(Mutex::new(inner)) }
    }

    pub fn add(&mut self, sample: usize) {
        let mut m = self.inner.lock().unwrap();
        (*m).sample += sample;
    }

    pub fn rate(&self) -> f64 {
        (*self.inner.lock().unwrap()).rate
    }

    pub fn update(&mut self, time_in_ms: usize) {
        let mut m = self.inner.lock().unwrap();
        (*m).rate = ((*m).sample as f64) * 8.0 / (time_in_ms as f64);
        (*m).sample = 0;
    }
}

#[derive(Clone)]
pub struct LatencyMonitor {
    inner: Arc<Mutex<LatencyInner>>,
}

#[derive(Debug)]
struct LatencyInner {
    sample: Vec<f64>,
    rate: f64,
}

impl LatencyMonitor {
    pub fn new() -> LatencyMonitor {
        let inner = LatencyInner {
            sample: Vec::with_capacity(32),
            rate: 0.0,
        };
        LatencyMonitor { inner: Arc::new(Mutex::new(inner)) }
    }

    pub fn add(&mut self, sample: f64) {
        let mut m = self.inner.lock().unwrap();
        (*m).sample.push(sample);
    }

    pub fn rate(&self) -> f64 {
        (*self.inner.lock().unwrap()).rate
    }

    pub fn update(&mut self) {
        let mut m = self.inner.lock().unwrap();
        (*m).rate = (*m).sample.iter().sum::<f64>() / (*m).sample.len() as f64;
        (*m).sample.clear();
    }
}
