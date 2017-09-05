use futures::{Async, Future, Poll, Stream};
use futures::sync::oneshot::{self, Receiver, Sender};
use std::time::Duration;
use tokio_timer::{Sleep, Timer, TimerError};

/// A stream representing notifications at fixed interval that can be stopped.
#[derive(Debug)]
pub struct Interval {
    sleep: Sleep,
    duration: Duration,
    rx: Receiver<()>,
}

/// Create a new interval and a control channel to stop it
pub fn new(timer: Timer, duration: Duration) -> (Interval, Sender<()>) {
    let (tx, rx) = oneshot::channel();
    let interval = Interval {
        sleep: timer.sleep(duration),
        duration: duration,
        rx: rx,
    };
    (interval, tx)
}

impl Stream for Interval {
    type Item = ();
    type Error = TimerError;

    fn poll(&mut self) -> Poll<Option<()>, TimerError> {
        if Ok(Async::Ready(())) == self.rx.poll() {
            // Cancel this stream
            return Ok(Async::Ready(None));
        }

        let _ = try_ready!(self.sleep.poll());
        // Reset the timeout
        self.sleep = self.sleep.timer().sleep(self.duration);
        Ok(Async::Ready(Some(())))
    }
}
