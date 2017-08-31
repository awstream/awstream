//! Socket implements `Sink` trait that can keep track of the delivered bytes
//! for bandwidth estimation.

use super::{AsCodec, AsDatum};
use futures::{Async, AsyncSink, Poll, Sink, StartSend};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_core::net::TcpStream;
use tokio_io::AsyncRead;
use tokio_io::codec::Framed;

#[derive(Debug)]
pub struct Socket {
    inner: Framed<TcpStream, AsCodec>,
    bytes: Arc<AtomicUsize>,
    last_item_size: usize,
}

impl Socket {
    pub fn new(tcp: TcpStream) -> (Socket, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        (
            Socket {
                inner: tcp.framed(AsCodec::default()),
                bytes: counter.clone(),
                last_item_size: 0,
            },
            counter,
        )
    }
}

impl Sink for Socket {
    type SinkItem = AsDatum;
    type SinkError = ();

    fn start_send(&mut self, item: AsDatum) -> StartSend<AsDatum, Self::SinkError> {
        self.last_item_size = item.len();
        match self.inner.start_send(item) {
            Ok(AsyncSink::Ready) => {
                info!(
                    "start sending new item, add {} to the counter",
                    self.last_item_size
                );
                self.bytes.fetch_add(self.last_item_size, Ordering::SeqCst);
                Ok(AsyncSink::Ready)
            }
            Ok(AsyncSink::NotReady(t)) => Ok(AsyncSink::NotReady(t)),
            Err(_e) => Err(()),
        }
    }


    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        match self.inner.poll_complete() {
            Ok(Async::Ready(_t)) => Ok(Async::Ready(())),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_e) => Err(()),
        }
    }
}
