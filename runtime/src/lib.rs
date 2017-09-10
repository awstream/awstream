//! AWStream: adaptive streaming for low-latency wide-area communication.
//!
//! This crate implements the runtime system described in "AWStream: Adaptive
//! Wide-Area Streaming Analytics", Figure 5.
//!
//! Key data structures are prefixed with `As`.
#![recursion_limit = "1024"]
#![deny(missing_docs)]

extern crate toml;
extern crate average;
extern crate bincode;
extern crate byteorder;
extern crate bytes;
extern crate chrono;
extern crate csv;
#[macro_use]
extern crate error_chain;
extern crate evaluation;
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

/// A convenience macro for working with `io::Result<T>` from the `Read` and
/// `Write` traits.
///
/// This macro takes `io::Result<T>` as input, and returns `T` as the output. If
/// the input type is of the `Err` variant, then `Poll::NotReady` is returned if
/// it indicates `WouldBlock` or otherwise `Err` is returned.
#[macro_export]
macro_rules! try_nb {
    ($e:expr) => (match $e {
        Ok(t) => t,
        Err(ref e) if e.kind() == ::std::io::ErrorKind::WouldBlock => {
            return Ok(::futures::Async::NotReady)
        }
        Err(e) => return Err(e.into()),
    })
}

// mod online;
mod adaptation;
mod analytics;
mod bw_monitor;
mod controller;
mod errors;
mod interval;
mod profile;
mod queue;
mod setting;
mod socket;
mod source;
mod utils;
mod video;
pub mod client;
pub mod server;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::{BufMut, BytesMut};
use errors::*;
use profile::SimpleProfile;
pub use setting::Setting;
use std::io::{self, Cursor};
use std::mem;
use tokio_io::codec::{Decoder, Encoder};

/// Actions for adaptation.
pub enum AdaptAction {
    /// Adapts to a designated bandwidth in kbps.
    ToRate(f64),

    /// Decreases the adaptation level.
    DecreaseDegradation,

    /// Starts probing with target bandwidth in kbps.
    StartProbe(f64),

    /// Increases probe pace.
    IncreaseProbePace,

    /// Stops the probing.
    StopProbe,
}

/// The core trait that a struct should react by changing levels.
pub trait Adapt {
    /// Adapts to a bandwidth constraint.
    fn adapt(&mut self, bandwidth: f64);

    /// Decreases the current degradation level.
    fn dec_degradation(&mut self);

    /// Period
    fn period_in_ms(&self) -> u64;

    /// Report the current level.
    fn current_level(&self) -> usize;

    /// Return a simple profile
    fn simple_profile(&self) -> SimpleProfile;
}

/// For experiment
pub trait Experiment {
    /// Return the size of next datum and its index.
    fn next_datum(&mut self) -> (usize, usize);
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
    /// Creates a new `AsDatum` object for live data.
    pub fn new(level: usize, frame_num: usize, data: Vec<u8>) -> AsDatum {
        let now = chrono::Utc::now();
        let mut d = AsDatum {
            t: AsDatumType::Live(level, frame_num),
            ts: now,
            mem: data,
            len: 0,
        };
        d.update_len();
        d
    }

    /// Creates a new `AsDatum` object for probing.
    pub fn bw_probe(size: usize) -> AsDatum {
        let now = chrono::Utc::now();
        let mut d = AsDatum {
            t: AsDatumType::Dummy,
            ts: now,
            mem: vec![0; size],
            len: 0,
        };
        d.update_len();
        d
    }

    /// Creates a new `AsDatum` object for probing RTT.
    pub fn latency_probe() -> AsDatum {
        let now = chrono::Utc::now();
        let mut d = AsDatum {
            t: AsDatumType::LatencyProbe,
            ts: now,
            mem: vec![0; 0],
            len: 0,
        };
        d.update_len();
        d
    }

    /// Creates a new `AsDatum` object for acknowledgement.
    pub fn ack(rr: ReceiverReport) -> Result<AsDatum> {
        let now = chrono::Utc::now();
        let mem = rr.to_mem()?;
        let mut d = AsDatum {
            t: AsDatumType::ReceiverCongest,
            ts: now,
            mem: mem,
            len: 0,
        };
        d.update_len();
        Ok(d)
    }

    fn update_len(&mut self) {
        // effective length includes the encoding of the length itself.
        self.len = bincode::serialized_size(self);
    }

    /// Returns the effective length (in bytes) for network transmission.
    pub fn net_len(&self) -> usize {
        // effective length includes the encoding of the length itself.
        self.len as usize + mem::size_of::<u64>()
    }

    /// Returns the datum type.
    pub fn datum_type(&self) -> AsDatumType {
        self.t
    }

    /// Return the serialized length of this data structure
    pub fn len(&self) -> usize {
        self.len as usize
    }
}

impl ::std::fmt::Display for AsDatum {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self.t {
            AsDatumType::Live(level, frame_num) => {
                f.debug_struct("AsDatum::Live")
                    .field("level", &level)
                    .field("frame_num", &frame_num)
                    .field("ts", &self.ts)
                    .field("mem_length", &self.mem.len())
                    .field("len", &self.len())
                    .finish()
            }
            AsDatumType::Raw => write!(f, "raw data: {}", self.len),
            AsDatumType::Dummy => write!(f, "probe data: {}", self.len),
            AsDatumType::LatencyProbe => write!(f, "probe latency"),
            AsDatumType::ReceiverCongest => write!(f, "receiver congest"),
        }
    }
}

/// Datum type.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsDatumType {
    /// Actual live data (meaningful), with (level, frame_num)
    Live(usize, usize),

    /// Raw data (used for online profiling).
    Raw,

    /// Dummy (bandwidth) probe packet.
    Dummy,

    /// Rtt probe packet.
    LatencyProbe,

    /// Signals that the receiver detects congestion.
    ReceiverCongest,
}

#[derive(Serialize, Deserialize)]
/// Statistics report from the receiver side.
pub struct ReceiverReport {
    latency: f64,
    goodput: f64,
    throughput: f64,
}

impl ReceiverReport {
    /// Creates
    pub fn new(latency: f64, goodput: f64, throughput: f64) -> Self {
        ReceiverReport {
            latency: latency,
            goodput: goodput,
            throughput: throughput,
        }
    }

    /// Decode from memory
    pub fn from_mem(mem: &Vec<u8>) -> Result<ReceiverReport> {
        let report = bincode::deserialize(&mem[..])?;
        Ok(report)
    }

    /// Encode into memory
    pub fn to_mem(&self) -> Result<Vec<u8>> {
        let mem = bincode::serialize(&self, bincode::Infinite)?;
        Ok(mem)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
/// `AsDatum` is the core data object for streaming over the network.
pub struct AsDatum {
    /// The type of this datum.
    t: AsDatumType,

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
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<AsDatum>> {
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
    type Error = Error;

    fn encode(&mut self, d: AsDatum, buf: &mut BytesMut) -> Result<()> {
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
        let d = AsDatum::new(0, 0, String::from("Hello").into_bytes());
        let expected_len = d.net_len();
        let expected = d.clone();
        let mut buf = bytes::BytesMut::new();
        let mut codec = AsCodec::default();
        codec.encode(d, &mut buf).unwrap();

        // Check the length is the same
        assert_eq!(buf.len(), expected_len);

        // Check that decode is succesful length
        let decoded = codec.decode(&mut buf);
        assert_eq!(decoded.unwrap().unwrap(), expected);
    }
}
