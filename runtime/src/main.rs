//! Using a transport directly
//!
//! This example illustrates a use case where the protocol isn't request /
//! response oriented. In this case, the connection is established, and "log"
//! entries are streamed to the remote.
//!
//! Given that the use case is not request / response oriented, it doesn't make
//! sense to use `tokio-proto`. Instead, we use the transport directly.

extern crate awstream;
extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate bytes;

use awstream::*;

use futures::{Future, Sink, Stream, stream};
use std::{io, str, thread};
use std::time::Duration;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;

use tokio_io::codec::Encoder;

/// Run the server. The server will simply listen for new connections, receive
/// strings, and write them to STDOUT.
///
/// The function will block until the server is shutdown.
pub fn server() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let remote_addr = "127.0.0.1:14566".parse().unwrap();

    let listener = TcpListener::bind(&remote_addr, &handle).unwrap();

    // Accept all incoming sockets
    let server = listener.incoming().for_each(move |(socket, _)| {
        let transport = socket.framed(AsCodec::default());

        let process_connection = transport.for_each(|line| {
            println!("GOT: {:?}", line);
            Ok(())
        });

        // Spawn a new task dedicated to processing the connection
        handle.spawn(process_connection.map_err(|_| ()));
        Ok(())
    });

    // Open listener
    core.run(server).unwrap();
}

pub fn main() {
    // Run the server in a dedicated thread
    thread::spawn(|| server());

    // Wait a moment for the server to start...
    thread::sleep(Duration::from_millis(100));

    // Connect to the remote
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let remote_addr = "127.0.0.1:14566".parse().unwrap();

    let work = TcpStream::connect(&remote_addr, &handle);
    let tcp = core.run(work).unwrap();

    let transport = tcp.framed(AsCodec::default());
    // We're just going to send a few dummy objects
    let lines_to_send: Vec<Result<AsDatum, io::Error>> =
        vec![Ok(AsDatum::new(String::from("Hello").into_bytes())),
             Ok(AsDatum::new(String::from("world").into_bytes()))];


    let d = AsDatum::new(String::from("Hello").into_bytes());
    let mut buf = bytes::BytesMut::new();
    let mut codec = AsCodec::default();
    codec.encode(d, &mut buf).unwrap();

    // Send all the messages to the remote.
    let work = transport.send_all(stream::iter(lines_to_send));
    core.run(work).unwrap();

    // Wait a bit to make sure that the server had time to receive the lines and
    // print them to STDOUT
    thread::sleep(Duration::from_millis(100));
}
