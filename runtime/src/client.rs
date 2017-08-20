//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.
//!
//! The event is one of the following:
//!  * `Source` timeouts and sends an `AsDatum` item
//!  * `Socket` finishes sending the previous item
//!  * `AC` timeous and returns a congestion status

use super::socket;
use futures::{self, Stream, Future, Sink};
use super::AsDatum;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_timer::Timer;
use tokio_io::AsyncRead;

enum Event {
    MonitorTimer,
    Socket,
    SourceDatum,
}

/// Run client
pub fn run() {
    use std::cell::RefCell;

    // Setting up the reactor core
    let mut core = Core::new().unwrap();

    // Creates the TCP connection and a transport
    let handle = core.handle();
    let remote_addr = "127.0.0.1:14566".parse().unwrap();
    let work = TcpStream::connect(&remote_addr, &handle);
    let tcp = core.run(work).unwrap();
    let socket = socket::Socket::new(tcp);

    let (tx, rx) = futures::sync::mpsc::unbounded();

    // monitor is a timer task
    let monitor = Timer::default()
        .interval(::std::time::Duration::from_millis(500))
        .map(|_| {
            // We perform monitor tasks, including reading the past bandwidth
            // and calling out to congestion controller if necessary.
            info!("timer fired");
            Event::MonitorTimer
        })
        .map_err(|_| ());

    let source = Timer::default()
        .interval(::std::time::Duration::from_millis(400))
        .map_err(|_| ())
        .and_then(|_| {
            // source is a timer task
            let data_to_send = AsDatum::new(vec![0; 1_024_0]);
            tx.clone().send(data_to_send).map_err(|_| ())
        })
        .map(|_| Event::SourceDatum)
        .map_err(|_| ());

    // We spawn a worker to handle all socket communication
    let handle = core.handle();
    let work = socket.send_all(rx).map(|_| ());
    handle.spawn(work);

    // Run the main loop: monitoring and source generating
    let all = monitor.select(source).for_each(|_| Ok(()));
    core.run(all).unwrap();
}
