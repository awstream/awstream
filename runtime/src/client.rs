//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.
//!
//! The event is one of the following:
//!  * `Source` timeouts and sends an `AsDatum` item
//!  * `Socket` finishes sending the previous item
//!  * `AC` timeous and returns a congestion status

use super::AsDatum;
use super::controller;
use super::socket;
use futures::{self, Future, Sink, Stream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_timer as timer;

enum Event {
    MonitorTimer,
    Socket,
    SourceDatum,
}

/// Run client
pub fn run() {
    let src_bytes = Arc::new(AtomicUsize::new(0));
    let src_bytes_clone = src_bytes.clone();
    let out_bytes = Arc::new(AtomicUsize::new(0));

    // Setting up the reactor core
    let mut core = Core::new().unwrap();

    // Creates the TCP connection and the socket
    let remote_addr = "127.0.0.1:14566".parse().unwrap();
    let handle = core.handle();
    let work = TcpStream::connect(&remote_addr, &handle);
    let tcp = core.run(work).unwrap();
    let socket = socket::Socket::new(tcp, out_bytes.clone());

    let (tx, rx) = futures::sync::mpsc::unbounded();

    // monitor is a timer task
    let monitor = controller::Monitor::new(src_bytes.clone(), out_bytes.clone())
        .map(move |_| Event::MonitorTimer)
        .map_err(|_| ());

    let mut counter = 0;
    let source = timer::Timer::default()
        .interval(::std::time::Duration::from_millis(200))
        .map_err(|_| ())
        .for_each(move |_| {
            // source is a timer task
            counter += 1;
            let data_to_send = AsDatum::new(vec![0; 1_024_000]);
            info!("add new data {}", data_to_send.len());
            src_bytes_clone.fetch_add(data_to_send.len(), Ordering::SeqCst);
            tx.clone().send(data_to_send).map(|_| ()).map_err(|_| ())
        })
        .map_err(|_| ());

    core.handle().spawn(source);

    // We spawn a worker to handle all socket communication
    let handle = core.handle();
    let work = socket.send_all(rx).map(|_| ());
    handle.spawn(work);

    // Run the main loop: monitoring and source generating
    let all = monitor.for_each(|_| Ok(()));
    core.run(all).unwrap();
}
