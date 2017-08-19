//! AWStream: adaptive streaming for low-latency wide-area communication.
//!
//! This crate implements the runtime system described in "AWStream: Adaptive
//! Wide-Area Streaming Analytics", Figure 5.
//!
//! Key data structures are prefixed with `As`.
#![recursion_limit = "1024"]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate bytes;
extern crate chrono;
extern crate byteorder;
#[macro_use]
extern crate serde_derive;
extern crate bincode;

// mod source;
// mod socket;
// mod receiver;
// mod analytics;
// mod online;
// mod controller;

// use bytes::BufMut;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::{BufMut, BytesMut};
use chrono::{DateTime, Utc};
use std::io::{self, Cursor};
use std::mem;
use tokio_io::codec::{Decoder, Encoder};

#[derive(Debug)]
enum CodecState {
    Len,
    Payload { len: u64 },
}

impl Default for AsCodec {
    fn default() -> Self {
        AsCodec { state: CodecState::Len }
    }
}

/// A wrapping codec to use Tokio.
pub struct AsCodec {
    state: CodecState,
}

impl AsDatum {
    /// Creates a new `AsDatum` object.
    pub fn new(data: Vec<u8>) -> AsDatum {
        let mut d = AsDatum {
            level: None,
            ts: None,
            mem: data,
            len: 0,
        };
        let len = bincode::serialized_size(&d);
        d.len = len;
        d
    }

    /// Return the serialized length of this data structure
    pub fn len(&self) -> usize {
        self.len as usize
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// `AsDatum` is the core data object for streaming over the network.
pub struct AsDatum {
    /// The degradation level associated with this data. Optional, and when set,
    /// it will be encoded.
    level: Option<usize>,

    /// Timestamp associated with the sender. We use
    ts: Option<DateTime<Utc>>,

    /// The pointer to the actual memory. We only hold a reference to the memory
    /// to facilitate zero-copy network programming. Underlying the hood, it
    /// uses reference counting for safe free.
    mem: Vec<u8>,

    /// The size of serialized version of this data structure (except this
    /// field). We use this field as a cache to avoid repeated call for
    /// serialization.
    #[serde(skip)]
    len: u64,
}

impl Decoder for AsCodec {
    type Item = AsDatum;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<AsDatum>, io::Error> {
        trace!("Decode: {:?}", buf);
        loop {
            match self.state {
                CodecState::Len if buf.len() < mem::size_of::<u64>() => {
                    trace!("--> Buf len is {}; waiting for 8 to parse len.", buf.len());
                    return Ok(None);
                }
                CodecState::Len => {
                    let mut len_buf = buf.split_to(mem::size_of::<u64>());
                    let len = Cursor::new(&mut len_buf).read_u64::<BigEndian>()?;
                    trace!("--> Parsed len = {} from {:?}", len, len_buf);
                    self.state = CodecState::Payload { len: len };
                }
                CodecState::Payload { len, .. } if buf.len() < len as usize => {
                    trace!(
                        "--> Buf len is {}; waiting for {} to parse packet length.",
                        buf.len(),
                        len
                    );
                    return Ok(None);
                }
                CodecState::Payload { len } => {
                    let payload = buf.split_to(len as usize);
                    self.state = CodecState::Len;
                    let mut datum: AsDatum =
                        bincode::deserialize_from(&mut Cursor::new(payload), bincode::Infinite)
                            .map_err(|deserialize_err| {
                                io::Error::new(io::ErrorKind::Other, deserialize_err)
                            })?;
                    datum.len = len;
                    return Ok(Some(datum));
                }
            }
        }
    }
}

impl Encoder for AsCodec {
    type Item = AsDatum;
    type Error = io::Error;

    fn encode(&mut self, d: AsDatum, buf: &mut BytesMut) -> Result<(), io::Error> {
        let payload_size = d.len;
        let message_size = mem::size_of::<u64>() + payload_size as usize;
        buf.reserve(message_size);

        // First write payload size
        buf.put_u64::<BigEndian>(payload_size);
        bincode::serialize_into(&mut buf.writer(), &d, bincode::Infinite)
            .map_err(|serialize_err| {
                io::Error::new(io::ErrorKind::Other, serialize_err)
            })?;

        trace!("Encoded buffer: {:?}", buf);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
