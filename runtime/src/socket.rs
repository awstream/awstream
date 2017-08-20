//! Socket implements `Sink` trait that can keep track of the delivered bytes
//! for bandwidth estimation.

use futures::{Stream, Sink, StartSend, Poll};
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
        info!("start sending item with len: {}", item.len);
        self.inner.start_send(item).map_err(|_| ())
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_complete().map_err(|_| ())
    }
}
