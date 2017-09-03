//! Socket implements `Sink` trait that can keep track of the delivered bytes
//! for bandwidth estimation.

use super::{AsCodec, AsDatum};
use bytes::BytesMut;
use futures::{Async, AsyncSink, Poll, Sink, StartSend};
use io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_core::net::TcpStream;
use tokio_io::codec::Encoder;
use std::io::Write;

#[derive(Debug)]
pub struct Socket {
    net: TcpStream,
    encoder: AsCodec,

    bytes: Arc<AtomicUsize>,
    last_item_size: usize,

    buffer: BytesMut,
}

impl Socket {
    pub fn new(tcp: TcpStream) -> (Socket, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        (
            Socket {
                net: tcp,
                encoder: AsCodec::default(),
                bytes: counter.clone(),
                last_item_size: 0,

                buffer: BytesMut::with_capacity(INITIAL_CAPACITY),
            },
            counter,
        )
    }
}

const INITIAL_CAPACITY: usize = 8 * 1024;
const BACKPRESSURE_BOUNDARY: usize = INITIAL_CAPACITY;

impl Sink for Socket {
    type SinkItem = AsDatum;
    type SinkError = io::Error;

    fn start_send(&mut self, item: AsDatum) -> StartSend<AsDatum, Self::SinkError> {
        // If the buffer is already over 8KiB, then attempt to flush it. If
        // after flushing it's *still* over 8KiB, then apply backpressure
        // (reject the send).
        if self.buffer.len() >= BACKPRESSURE_BOUNDARY {
            try!(self.poll_complete());

            if self.buffer.len() >= BACKPRESSURE_BOUNDARY {
                return Ok(AsyncSink::NotReady(item));
            }
        }

        try!(self.encoder.encode(item, &mut self.buffer));

        Ok(AsyncSink::Ready)
    }


    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        trace!("flushing socket");
        while !self.buffer.is_empty() {
            trace!("writing; remaining={}", self.buffer.len());

            let n = try_nb!(self.net.write(&self.buffer));

            self.bytes.fetch_add(n, Ordering::SeqCst);
            info!("complete sending item with size {}", n);

            if n == 0 {
                return Err(
                    io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write frame to transport",
                    ).into(),
                );
            }

            let _ = self.buffer.split_to(n);
        }

        // Try flushing the underlying IO
        try_nb!(self.net.flush());

        trace!("socket packet flushed");
        return Ok(Async::Ready(()));
    }
}
