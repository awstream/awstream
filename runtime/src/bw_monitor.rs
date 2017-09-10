use errors::*;
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

    pub fn add(&mut self, sample: usize) -> Result<()> {
        let mut m = self.inner.lock()?;
        (*m).sample += sample;
        Ok(())
    }

    pub fn rate(&self) -> Result<f64> {
        let m = self.inner.lock()?;
        Ok((*m).rate)
    }

    pub fn update(&mut self, time_in_ms: usize) -> Result<()> {
        let mut m = self.inner.lock()?;
        (*m).rate = ((*m).sample as f64) * 8.0 / (time_in_ms as f64);
        (*m).sample = 0;
        Ok(())
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

    pub fn add(&mut self, sample: f64) -> Result<()> {
        let mut m = self.inner.lock()?;
        (*m).sample.push(sample);
        Ok(())
    }

    pub fn rate(&self) -> Result<f64> {
        let m = self.inner.lock()?;
        Ok(m.rate)
    }

    pub fn update(&mut self) -> Result<()> {
        let mut m = self.inner.lock()?;
        (*m).rate = (*m).sample.iter().sum::<f64>() / (*m).sample.len() as f64;
        (*m).sample.clear();
        Ok(())
    }
}
