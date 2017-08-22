//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.

use super::controller::Monitor;
use super::socket::Socket;
use super::video::VideoSource;
use super::adaptation::{Adaptation, Action};
use futures::{Future, Sink, Stream};
use super::source::TimerSource;
use std::time::Duration;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;

/// Run client
pub fn run() {
    // Setting up the reactor core
    let mut core = Core::new().unwrap();

    // Creates the TCP connection (this is synchronous!)
    let remote_addr = "127.0.0.1:14566".parse().unwrap();
    let handle = core.handle();
    let work = TcpStream::connect(&remote_addr, &handle);
    let tcp = core.run(work).unwrap();

    let video_source = VideoSource::new("/tmp/x.csv", "/tmp/y.csv");
    // First we create source
    let (_level_ctrl, source, src_bytes) =
        TimerSource::spawn(video_source, Duration::from_millis(200), core.handle());

    // Then we create sink (socket)
    let (socket, out_bytes) = Socket::new(tcp);

    // Next, we forward all source data to socket
    let socket_work = socket.send_all(source).map(|_| ());
    core.handle().spawn(socket_work);

    // Lastly, we create adaptation
    let mut adaptation = Adaptation::default();

    // monitor is a timer task
    let monitor = Monitor::new(src_bytes, out_bytes)
        .map(|signal| {
            let action = adaptation.transit(signal);
            match action {
                Action::AdjustConfig => {
                    // level_ctrl.send(3),
                    info!("adjusting config {:?}", action);
                }
                _ => {
                    info!("action {:?}", action);
                }
            }
        })
        .map_err(|_| ())
        .for_each(|_| Ok(()));

    core.run(monitor).unwrap();
}
