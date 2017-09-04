//! Utility structures and functions.

use average::{MeanWithError, Quantile};

const ALPHA: f64 = 0.8;

pub struct ExponentialSmooth {
    val: f64,
}

impl ExponentialSmooth {
    pub fn default() -> Self {
        ExponentialSmooth { val: 0.0 }
    }

    pub fn add(&mut self, sample: f64) {
        self.val = self.val * ALPHA + sample * (1.0 - ALPHA);
    }

    pub fn val(&self) -> f64 {
        self.val
    }
}

pub struct StreamingStat {
    buffer: Vec<f64>,
    pos: usize,
    capacity: usize,
}

impl StreamingStat {
    pub fn with_capacity(size: usize) -> Self {
        assert!(size > 0);
        StreamingStat {
            pos: 0,
            capacity: size,
            buffer: vec![0.0; size],
        }
    }

    pub fn add(&mut self, sample: f64) {
        self.buffer[self.pos] = sample;
        self.pos += 1;
        if self.pos == self.capacity {
            self.pos = 0;
        }
    }

    pub fn sum(&self) -> f64 {
        trace!("for sum, consumed {:?}", self.buffer);
        self.buffer.iter().sum()
    }

    pub fn mean(&self) -> (f64, f64) {
        trace!("for mean, consumed {:?}", self.buffer);
        let mut m = MeanWithError::default();
        self.buffer.iter().map(|&i| m.add(i)).count();
        (m.mean(), m.error())
    }

    pub fn p99(&self) -> f64 {
        trace!("for p99, consumed {:?}", self.buffer);
        let mut q = Quantile::new(0.99);
        self.buffer.iter().map(|&i| q.add(i)).count();
        q.quantile()
    }
}
