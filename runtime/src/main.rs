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
extern crate env_logger;
extern crate chrono;
#[macro_use]
extern crate log;

use awstream::*;
use futures::{Future, Stream};
use std::{env, str, thread};
use std::time::Duration;
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_io::AsyncRead;

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
            trace!("GOT: {:?}", line);
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
    let format = |record: &log::LogRecord| {
        let t = chrono::Utc::now();
        format!(
            "{} {}:{}: {}",
            t.format("%Y-%m-%d %H:%M:%S.%f").to_string(),
            record.level(),
            record.location().module_path(),
            record.args()
        )
    };

    let mut builder = env_logger::LogBuilder::new();
    builder.format(format);
    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.init().unwrap();

    // Run the server in a dedicated thread
    thread::spawn(|| server());

    // Wait a moment for the server to start...
    thread::sleep(Duration::from_millis(100));

    // Client runs
    client::run();
}
