//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.
//!
//! The event is one of the following:
//!  * `Source` timeouts and sends an `AsDatum` item
//!  * `Socket` finishes sending the previous item
//!  * `AC` timeous and returns a congestion status

use futures::{Future, Stream, Sink};
use super::{AsCodec, AsDatum};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;
use tokio_timer::Timer;

/// Run client
pub fn run() {
    // Setting up the reactor core
    let mut core = Core::new().unwrap();

    // Creates the TCP connection and a transport
    let handle = core.handle();
    let remote_addr = "127.0.0.1:14566".parse().unwrap();
    let work = TcpStream::connect(&remote_addr, &handle);
    let tcp = core.run(work).unwrap();
    let transport = tcp.framed(AsCodec::default());

    // We're just going to send a few dummy objects
    let data_send = AsDatum::new(String::from("Hello").into_bytes());

    // Test socket by send an data item
    let socket = transport.send(data_send);

    // monitor is a timer task
    let monitor = Timer::default()
        .interval(::std::time::Duration::from_millis(500))
        .for_each(|_| {
            println!("timer fired");
            Ok(())
        })
        .map_err(|_| ());

    core.run(socket).unwrap();
    core.run(monitor).unwrap();
}
