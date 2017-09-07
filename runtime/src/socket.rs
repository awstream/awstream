//! Socket implements `Sink` trait that can keep track of the delivered bytes
//! for bandwidth estimation.

use errors::*;
use super::{AsCodec, AsDatum};
use bytes::BytesMut;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use std::{fmt, io};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_core::net::TcpStream;
use tokio_io::AsyncRead;
use tokio_io::codec::{Decoder, Encoder};
use tokio_io::io::WriteHalf;

#[derive(Debug)]
pub struct Socket {
    net: WriteHalf<TcpStream>,
    encoder: AsCodec,

    bytes: Arc<AtomicUsize>,
    last_item_size: usize,

    buffer: BytesMut,
}

impl Socket {
    pub fn new(tcp: WriteHalf<TcpStream>) -> (Socket, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        let socket = Socket {
            net: tcp,
            encoder: AsCodec::default(),
            bytes: counter.clone(),
            last_item_size: 0,

            buffer: BytesMut::with_capacity(INITIAL_CAPACITY),
        };
        (socket, counter)
    }
}

const INITIAL_CAPACITY: usize = 32 * 1_024;
const BACKPRESSURE_BOUNDARY: usize = INITIAL_CAPACITY;

impl Sink for Socket {
    type SinkItem = AsDatum;
    type SinkError = Error;

    fn start_send(&mut self, item: AsDatum) -> StartSend<AsDatum, Error> {
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

/// A `Stream` of messages decoded from an `AsyncRead`.
pub struct FramedRead<T, D> {
    inner: T,
    decoder: D,
    eof: bool,
    is_readable: bool,
    buffer: BytesMut,
}

const READ_CAPACITY: usize = 8 * 1024;

// ===== impl FramedRead =====

impl<T, D> FramedRead<T, D>
where
    T: AsyncRead,
    D: Decoder,
{
    /// Creates a new `FramedRead` with the given `decoder`.
    pub fn new(inner: T, decoder: D) -> FramedRead<T, D> {
        FramedRead {
            inner: inner,
            decoder: decoder,
            eof: false,
            is_readable: false,
            buffer: BytesMut::with_capacity(READ_CAPACITY),
        }
    }
}

impl<T, D> Stream for FramedRead<T, D>
where
    T: AsyncRead,
    D: Decoder,
{
    type Item = D::Item;
    type Error = D::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            // Repeatedly call `decode` or `decode_eof` as long as it is
            // "readable". Readable is defined as not having returned `None`. If
            // the upstream has returned EOF, and the decoder is no longer
            // readable, it can be assumed that the decoder will never become
            // readable again, at which point the stream is terminated.
            if self.is_readable {
                if self.eof {
                    let frame = try!(self.decoder.decode_eof(&mut self.buffer));
                    return Ok(Async::Ready(frame));
                }

                trace!("attempting to decode a frame");

                if let Some(frame) = try!(self.decoder.decode(&mut self.buffer)) {
                    trace!("frame decoded from buffer");
                    return Ok(Async::Ready(Some(frame)));
                }

                self.is_readable = false;
            }

            assert!(!self.eof);

            // Otherwise, try to read more data and try again. Make sure we've
            // got room for at least one byte to read to ensure that we don't
            // get a spurious 0 that looks like EOF
            self.buffer.reserve(1);
            if 0 == try_ready!(self.inner.read_buf(&mut self.buffer)) {
                self.eof = true;
            }

            self.is_readable = true;
        }
    }
}

impl<T, D> fmt::Debug for FramedRead<T, D>
where
    T: fmt::Debug,
    D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FramedRead")
            .field("inner", &self.inner)
            .field("decoder", &self.decoder)
            .field("eof", &self.eof)
            .field("is_readable", &self.is_readable)
            .field("buffer", &self.buffer)
            .finish()
    }
}
