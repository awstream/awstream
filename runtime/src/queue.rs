//! Channel that relays messages.

use super::{AsDatum, AsDatumType};
use errors::*;
use futures::{Async, Poll, Stream};
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};

pub struct SenderCtl {
    inner: UnboundedSender<AsDatum>,
    counter: Arc<AtomicIsize>,
}

impl SenderCtl {
    pub fn new(tx: UnboundedSender<AsDatum>, counter: Arc<AtomicIsize>) -> Self {
        SenderCtl {
            inner: tx,
            counter: counter,
        }
    }
}

pub struct ReceiverCtl {
    inner: UnboundedReceiver<AsDatum>,
    counter: Arc<AtomicIsize>,
}

impl ReceiverCtl {
    pub fn new(rx: UnboundedReceiver<AsDatum>, counter: Arc<AtomicIsize>) -> Self {
        ReceiverCtl {
            inner: rx,
            counter: counter,
        }
    }
}

pub fn queue() -> (SenderCtl, ReceiverCtl) {
    let (tx, rx) = unbounded();
    let c = Arc::new(AtomicIsize::new(0));
    (
        SenderCtl::new(tx, c.clone()),
        ReceiverCtl::new(rx, c.clone()),
    )
}

impl SenderCtl {
    pub fn send(&self, datum: AsDatum) -> Result<()> {
        let q_len = self.counter.load(Ordering::SeqCst);
        if q_len > 0 {
            info!("queue built up");
        }

        if let AsDatumType::Live(_, _) = datum.datum_type() {
            self.counter.fetch_add(1, Ordering::SeqCst);
        }

        self.inner.unbounded_send(datum).map_err(|_| {
            Error::from_kind(ErrorKind::DataPlane)
        })
    }
}

impl Stream for ReceiverCtl {
    type Item = AsDatum;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<AsDatum>, ()> {
        let item = try_ready!(self.inner.poll());

        if let Some(ref datum) = item {
            if let AsDatumType::Live(_, _) = datum.datum_type() {
                self.counter.fetch_sub(1, Ordering::SeqCst);
            }
        }

        Ok(Async::Ready(item))
    }
}
