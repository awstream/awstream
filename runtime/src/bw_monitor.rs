use std::sync::{Arc, Mutex};

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

    pub fn update(&mut self, time_in_ms: usize) {
        let mut m = self.inner.lock().unwrap();
        (*m).rate = ((*m).sample as f64) * 8.0 / (time_in_ms as f64);
        (*m).sample = 0;
    }

    pub fn rate(&self) -> f64 {
        (*self.inner.lock().unwrap()).rate
    }
}
