//! Utility structures and functions.

use average::{MeanWithError, Quantile};

pub struct ExponentialSmooth {
    val: f64,
    alpha: f64,
}

impl ExponentialSmooth {
    pub fn new(alpha: f64) -> Self {
        ExponentialSmooth {
            val: 0.0,
            alpha: alpha,
        }
    }

    pub fn add(&mut self, sample: f64) {
        self.val = self.val * self.alpha + sample * (1.0 - self.alpha);
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
    pub fn new(init: f64, size: usize) -> Self {
        assert!(init != ::std::f64::NAN);
        assert!(size > 0);
        StreamingStat {
            pos: 0,
            capacity: size,
            buffer: vec![init; size],
        }
    }

    pub fn add(&mut self, sample: f64) {
        assert!(sample != ::std::f64::NAN);
        self.buffer[self.pos] = sample;
        self.pos += 1;
        if self.pos == self.capacity {
            self.pos = 0;
        }
    }

    pub fn min(&self) -> f64 {
        *(self.buffer
              .iter()
              .min_by(|a, b| a.partial_cmp(b).unwrap())
              .unwrap())
    }

    pub fn _sum(&self) -> f64 {
        trace!("for sum, consumed {:?}", self.buffer);
        self.buffer.iter().sum()
    }

    pub fn _mean(&self) -> (f64, f64) {
        trace!("for mean, consumed {:?}", self.buffer);
        let mut m = MeanWithError::default();
        self.buffer.iter().map(|&i| m.add(i)).count();
        (m.mean(), m.error())
    }

    pub fn _p99(&self) -> f64 {
        trace!("for p99, consumed {:?}", self.buffer);
        let mut q = Quantile::new(0.99);
        self.buffer.iter().map(|&i| q.add(i)).count();
        q.quantile()
    }
}
