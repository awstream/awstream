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
}

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
        }
    }
}

impl Stream for Monitor {
    type Item = Signal;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match try_ready!(self.timer.poll()) {
                Some(_t) => {
                    trace!("monitor timer ticks");
                    // timer fired, we check ingest_bytes and out_bytes
                    let produced = self.produced_bytes.swap(0, Ordering::SeqCst);
                    let consumed = self.consumed_bytes.swap(0, Ordering::SeqCst);

                    self.queued += produced - consumed;
                    self.rate.add(consumed as f64);

                    info!("rate: {:.3} kbps", self.rate.sum() * 8.0 / 1000.0);
                    if self.queued > 0 {
                        let rate = self.rate.sum();
                        let latency = self.queued as f64 / rate;
                        return Ok(Async::Ready(Some(Signal::QueueCongest(rate, latency))));
                    }
                }
                None => return Ok(Async::Ready(None)),
            }
        }
    }
}
