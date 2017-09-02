use adaptation::Signal;
use futures::{Async, Poll, Stream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio_timer::{self, Interval};
use utils::StreamingStat;

pub struct Monitor {
    /// Fires to estimate outgoing bandwidth and expected latency
    timer: Interval,

    /// My Reference to the data being generated.
    produced_bytes: Arc<AtomicUsize>,

    /// My Reference to the data being consumed.
    consumed_bytes: Arc<AtomicUsize>,

    /// The estimated consumption rate.
    rate: StreamingStat,

    /// Queued bytes.
    queued: usize,

    /// Empty counts.
    empty_count: usize,
}

/// QUEUE_EMPTY_REQUIRED * MONITOR_INTERVAL => 1 seconds for each Q_E
const QUEUE_EMPTY_REQUIRED: usize = 10;

const MONITOR_INTERVAL: u64 = 100;

impl Monitor {
    pub fn new(producer: Arc<AtomicUsize>, consumer: Arc<AtomicUsize>) -> Self {
        let timer = tokio_timer::wheel()
            .tick_duration(Duration::from_millis(50))
            .build()
            .interval(Duration::from_millis(MONITOR_INTERVAL));
        Monitor {
            timer: timer,
            produced_bytes: producer,
            consumed_bytes: consumer,

            // every 10 samples is every second
            rate: StreamingStat::with_capacity(10),

            queued: 0,
            empty_count: 0,
        }
    }

    fn react_to_timer(&mut self) -> Option<Signal> {
        trace!("monitor timer ticks");

        // timer fired, we check the produced and consumed bytes
        let produced = self.produced_bytes.swap(0, Ordering::SeqCst);
        let consumed = self.consumed_bytes.swap(0, Ordering::SeqCst);

        self.queued += produced - consumed;
        self.rate.add(consumed as f64);

        let rate = self.rate.sum() * 8.0 / 1000.0; // rate is kbps
        let latency = self.queued as f64 / rate; // queued is bytes
        info!(
            "rate: {:.3} kbps, latency: {:.3} ms",
            rate,
            latency * 1000.0
        );
        if latency > 0.1 {
            self.empty_count = 0;
            return Some(Signal::QueueCongest(rate, latency));
        } else {
            self.empty_count += 1;
            if self.empty_count > QUEUE_EMPTY_REQUIRED {
                return Some(Signal::QueueEmpty);
            }
        }
        return None;
    }
}

impl Stream for Monitor {
    type Item = Signal;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match try_ready!(self.timer.poll()) {
                Some(_t) => {
                    match self.react_to_timer() {
                        Some(s) => return Ok(Async::Ready(Some(s))),
                        None => {}
                    }
                }
                None => {
                    return Ok(Async::Ready(None));
                }
            }
        }
    }
}
