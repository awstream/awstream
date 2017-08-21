//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.

use super::controller::Monitor;
use super::socket::Socket;
use futures::{Future, Sink, Stream};
use source::TimerSource;
use std::time::Duration;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;

enum Event {
    MonitorTimer,
    Socket,
    SourceDatum,
}

/// Run client
pub fn run() {
    // Setting up the reactor core
    let mut core = Core::new().unwrap();

    // Creates the TCP connection (this is synchronous!)
    let remote_addr = "127.0.0.1:14566".parse().unwrap();
    let handle = core.handle();
    let work = TcpStream::connect(&remote_addr, &handle);
    let tcp = core.run(work).unwrap();

    // First we create source
    let (source, src_bytes) = TimerSource::spawn(Duration::from_millis(200), core.handle());

    // Then we create sink (socket)
    let (socket, out_bytes) = Socket::new(tcp);

    let socket_work = socket.send_all(source).map(|_| ());
    core.handle().spawn(socket_work);

    // monitor is a timer task
    let monitor = Monitor::new(src_bytes, out_bytes)
        .map(|_| Event::MonitorTimer)
        .map_err(|_| ())
        .for_each(|_| Ok(()));

    core.run(monitor).unwrap();
}
