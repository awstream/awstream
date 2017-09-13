use adaptation::Signal;
use errors::*;
use futures::{Async, Poll, Stream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio_timer::{self, Interval};
use utils::ExponentialSmooth;

const ALPHA_RATE: f64 = 0.9;

pub struct Monitor {
    /// Fires to estimate outgoing bandwidth and expected latency
    timer: Interval,

    /// My Reference to the data being generated.
    produced_bytes: Arc<AtomicUsize>,

    /// My Reference to the data being consumed.
    consumed_bytes: Arc<AtomicUsize>,

    /// The estimated consumption rate.
    rate: ExponentialSmooth,

    /// Queued bytes.
    queued: usize,

    /// Empty counts.
    empty_count: usize,

    /// Remembers if timer has fired or not. We delay `react_to_timer` to avoid
    /// the race with `socket`.
    timer_fired: bool,
}

/// QUEUE_EMPTY_REQUIRED * MONITOR_INTERVAL => 1 seconds for each Q_E
const QUEUE_EMPTY_REQUIRED: usize = 20;

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
            rate: ExponentialSmooth::new(0.5),
            queued: 0,
            empty_count: 0,
            timer_fired: false,
        }
    }

    fn react_to_timer(&mut self) -> Option<Signal> {
        trace!("monitor timer ticks");

        // timer fired, we check the produced and consumed bytes
        let produced = self.produced_bytes.swap(0, Ordering::SeqCst);
        let consumed = self.consumed_bytes.swap(0, Ordering::SeqCst);

        self.queued = self.queued + produced - consumed;
        self.rate.add(consumed as f64);

        // self.rate tracks the amount of bytes sent over the last
        // MONITOR_INTERVAL (in ms). The division results in kbps.
        let rate = self.rate.val() * 8.0 / (MONITOR_INTERVAL as f64);
        let latency = self.queued as f64 * 8.0 / rate; // queued is bytes
        info!(
            "queued: {:?} kbytes, rate: {:.1} kbps, latency: {:.1} ms",
            self.queued / 1000,
            rate,
            latency
        );
        if latency > 1.0 {
            self.empty_count = 0;
            return Some(Signal::QueueCongest(ALPHA_RATE * rate, latency));
        } else {
            self.empty_count += 1;
            if self.empty_count > QUEUE_EMPTY_REQUIRED {
                self.empty_count = 0;
                return Some(Signal::QueueEmpty);
            }
        }
        return None;
    }
}

impl Stream for Monitor {
    type Item = Signal;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // We use a loop here to filter items: if `react_to_timer` returns
        // `None`, the loop will continue and `try_ready` will return
        // `Ok(Async::NotReady). In this way, not every timer tick will trigger
        // a monitor event. This follows the implementation of
        // `futures::Stream::filter`.
        loop {
            if self.timer_fired {
                self.timer_fired = false;
                match self.react_to_timer() {
                    Some(s) => return Ok(Async::Ready(Some(s))),
                    None => {}
                }
            }
            match try_ready!(self.timer.poll()) {
                Some(_t) => {
                    self.timer_fired = true;
                    let task = ::futures::task::current();
                    task.notify();
                    return Ok(Async::NotReady);
                }
                None => {
                    return Ok(Async::Ready(None));
                }
            }
        }
    }
}
