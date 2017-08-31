//! AWStream: adaptive streaming for low-latency wide-area communication.
//!
//! This crate implements the runtime system described in "AWStream: Adaptive
//! Wide-Area Streaming Analytics", Figure 5.
//!
//! Key data structures are prefixed with `As`.
#![recursion_limit = "1024"]
#![deny(missing_docs)]
#![allow(dead_code)]

extern crate toml;
extern crate average;
extern crate bincode;
extern crate byteorder;
extern crate bytes;
extern crate chrono;
extern crate csv;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_timer;

mod profile;
mod setting;
pub use setting::Setting;
mod adaptation;
mod controller;
pub mod client;
pub mod server;
mod socket;
mod utils;
mod source;

mod video;
// mod receiver;
// mod analytics;
// mod online;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::{BufMut, BytesMut};
use profile::SimpleProfile;
use std::io::{self, Cursor};
use std::mem;
use tokio_io::codec::{Decoder, Encoder};

/// Signals about adaptation actions
pub enum AdaptSignal {
    /// Adapt to a designated rate
    ToRate(f64),

    /// Decrease the adaptation level
    DecreaseDegradation,
}

/// The core trait that a struct should react by changing levels.
pub trait Adapt {
    /// Adapts to a bandwidth constraint.
    fn adapt(&mut self, bandwidth: f64);

    /// Decreases the current degradation level.
    fn dec_degradation(&mut self);

    /// Report the current level.
    fn current_level(&self) -> usize;

    /// Return a simple profile
    fn simple_profile(&self) -> SimpleProfile;
}

/// For experiment
pub trait Experiment {
    /// Return the size of next datum.
    fn next_datum(&mut self) -> usize;
}

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

#[derive(Debug)]
/// A wrapping codec to use Tokio.
pub struct AsCodec {
    state: CodecState,
}

impl AsDatum {
    /// Creates a new `AsDatum` object.
    pub fn new(level: usize, data: Vec<u8>) -> AsDatum {
        let now = chrono::Utc::now();
        let mut d = AsDatum {
            level: level,
            ts: now,
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

impl ::std::fmt::Display for AsDatum {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "level: {:?}, ts: {:?}, mem (with size {}), len: {}",
            self.level,
            self.ts,
            self.mem.len(),
            self.len
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
/// `AsDatum` is the core data object for streaming over the network.
pub struct AsDatum {
    /// The degradation level associated with this data. Optional, and when set,
    /// it will be encoded.
    level: usize,

    /// The pointer to the actual memory. We only hold a reference to the memory
    /// to facilitate zero-copy network programming. Underlying the hood, it
    /// uses reference counting for safe free.
    mem: Vec<u8>,

    /// Timestamp associated with the sender. We use unix time at UTC.
    ts: chrono::DateTime<chrono::Utc>,

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
    use super::*;
    #[test]
    fn encode_decode_works() {
        let d = AsDatum::new(0, String::from("Hello").into_bytes());
        let expected = d.clone();
        let mut buf = bytes::BytesMut::new();
        let mut codec = AsCodec::default();
        codec.encode(d, &mut buf).unwrap();

        let decoded = codec.decode(&mut buf);
        assert_eq!(decoded.unwrap().unwrap(), expected);
    }
}
