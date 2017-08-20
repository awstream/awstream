//! Socket implements `Sink` trait that can keep track of the delivered bytes
//! for bandwidth estimation.

use futures::{Async, AsyncSink, Sink, StartSend, Poll};
use tokio_core::net::TcpStream;
use tokio_io::codec::Framed;
use tokio_io::AsyncRead;
use super::{AsDatum, AsCodec};

#[derive(Debug)]
pub struct Socket {
    inner: Framed<TcpStream, AsCodec>,
}

impl Socket {
    pub fn new(tcp: TcpStream) -> Socket {
        Socket { inner: tcp.framed(AsCodec::default()) }
    }
}

impl Sink for Socket {
    type SinkItem = AsDatum;
    type SinkError = ();

    fn start_send(&mut self, item: AsDatum) -> StartSend<AsDatum, Self::SinkError> {
        let len = item.len();
        match self.inner.start_send(item) {
            Ok(AsyncSink::Ready) => {
                info!("start sending item {}", len);
                Ok(AsyncSink::Ready)
            }
            Ok(AsyncSink::NotReady(t)) => {
                info!("failed to send, should notify");
                Ok(AsyncSink::NotReady(t))
            }
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
