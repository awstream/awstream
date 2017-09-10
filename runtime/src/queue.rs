//! Channel that relays messages.

use errors::*;
use futures::{Async, Poll, Stream};
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};

pub struct SenderCtl<T> {
    inner: UnboundedSender<T>,
    counter: Arc<AtomicIsize>,
}

impl<T> SenderCtl<T> {
    pub fn new(tx: UnboundedSender<T>, counter: Arc<AtomicIsize>) -> Self {
        SenderCtl {
            inner: tx,
            counter: counter,
        }
    }
}

pub struct ReceiverCtl<T> {
    inner: UnboundedReceiver<T>,
    counter: Arc<AtomicIsize>,
}

impl<T> ReceiverCtl<T> {
    pub fn new(rx: UnboundedReceiver<T>, counter: Arc<AtomicIsize>) -> Self {
        ReceiverCtl {
            inner: rx,
            counter: counter,
        }
    }
}

pub fn queue<T>() -> (SenderCtl<T>, ReceiverCtl<T>) {
    let (tx, rx) = unbounded();
    let c = Arc::new(AtomicIsize::new(0));
    (
        SenderCtl::new(tx, c.clone()),
        ReceiverCtl::new(rx, c.clone()),
    )
}

impl<T: ::std::any::Any> SenderCtl<T> {
    pub fn send(&self, msg: T) -> Result<()> {
        let q_len = self.counter.load(Ordering::SeqCst);
        if q_len > 0 {
            info!("queue built up");
        }

        self.counter.fetch_add(1, Ordering::SeqCst);
        self.inner.unbounded_send(msg).map_err(|_| {
            Error::from_kind(ErrorKind::DataPlane)
        })
    }
}

impl<T> Stream for ReceiverCtl<T> {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<T>, ()> {
        let item = try_ready!(self.inner.poll());
        self.counter.fetch_sub(1, Ordering::SeqCst);
        Ok(Async::Ready(item))
    }
}
