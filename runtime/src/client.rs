//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.

use super::{Adapt, AdaptSignal};
use super::adaptation::{Action, Adaptation};
use super::controller::Monitor;
use super::socket::Socket;
use super::source::TimerSource;
use super::video::VideoSource;
use futures::{Future, Sink, Stream};
use std::time::Duration;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use std::net::SocketAddr;

/// Run client
pub fn run(addr: &SocketAddr) {
    // Setting up the reactor core
    let mut core = Core::new().unwrap();

    // Creates the TCP connection (this is synchronous!)
    let handle = core.handle();
    let work = TcpStream::connect(addr, &handle);
    let tcp = core.run(work).unwrap();

    let profile_path = "/tmp/mot.profile.csv";

    let video_source = VideoSource::new("/tmp/mot.source.csv", profile_path);
    let mut profile = video_source.simple_profile();

    // First we create source
    let (level_ctrl, source, src_bytes) =
        TimerSource::spawn(video_source, Duration::from_millis(33), core.handle());

    // Then we create sink (socket)
    let (socket, out_bytes) = Socket::new(tcp);

    // Next, we forward all source data to socket
    let socket_work = socket.send_all(source).map(|_| ());
    core.handle().spawn(socket_work);

    // Lastly, we create adaptation
    let mut adaptation = Adaptation::default();

    // monitor is a timer task
    let monitor = Monitor::new(src_bytes, out_bytes)
        .skip(5)
        .map(|signal| {
            let action = adaptation.transit(signal, profile.is_max());
            match action {
                Action::NoOp => {}
                Action::AdjustConfig(rate) => {
                    profile.adjust_level(rate);
                    level_ctrl
                        .clone()
                        .send(AdaptSignal::ToRate(rate))
                        .wait()
                        .expect("failed to control source");
                    info!("adjusting config {:?}", action);
                }
                Action::AdvanceConfig => {
                    profile.advance_level();
                    level_ctrl
                        .clone()
                        .send(AdaptSignal::DecreaseDegradation)
                        .wait()
                        .expect("failed to control source");
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
