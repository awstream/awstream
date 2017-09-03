//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.

use super::{Adapt, AdaptSignal};
use super::adaptation::{Signal, Action, Adaptation};
use super::controller::Monitor;
use super::setting::Setting;
use super::socket::Socket;
use super::source::TimerSource;
use super::profile::SimpleProfile;
use super::video::VideoSource;
use futures::{Future, Sink, Stream};
use futures::sync::mpsc::UnboundedSender;
use std::net::SocketAddr;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use io;

/// Run client
pub fn run(setting: Setting) {
    // Setting up the reactor core
    let mut core = Core::new().unwrap();

    // Creates the TCP connection (this is synchronous!)
    let handle = core.handle();
    let ip = setting.server.parse().unwrap();
    let address = SocketAddr::new(ip, setting.port);
    let work = TcpStream::connect(&address, &handle);
    let tcp = core.run(work).unwrap();

    let video_source = VideoSource::new(setting.source_path, setting.profile_path);
    let mut profile = video_source.simple_profile();

    // First we create source
    let handle = core.handle();
    let (src_ctrl, source, src_bytes, probe_done) = TimerSource::spawn(video_source, handle);

    // Then we create sink (socket)
    let (socket, out_bytes) = Socket::new(tcp);

    // Next, we forward all source data to socket
    let s = source.map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "failed to receive"));
    let socket_work = socket.send_all(s).map(|_| ()).map_err(|_| ());
    core.handle().spawn(socket_work);

    // Lastly, we create adaptation
    let mut adaptation = Adaptation::default();

    // monitor is a timer task
    let monitor = Monitor::new(src_bytes, out_bytes, probe_done)
        .skip(5)
        .map(|signal| {
            core_adapt(signal, &mut adaptation, &mut profile, src_ctrl.clone())
        })
        .map_err(|_| ())
        .for_each(|_| Ok(()));

    core.run(monitor).unwrap();
}

fn block_send(tx: UnboundedSender<AdaptSignal>, item: AdaptSignal) {
    let errmsg = "failed to control source";
    tx.send(item).wait().expect(&errmsg);
}

fn core_adapt(
    signal: Signal,
    adaptation: &mut Adaptation,
    profile: &mut SimpleProfile,
    src_ctrl: UnboundedSender<AdaptSignal>,
) {
    let action = adaptation.transit(signal, profile.is_max());
    match action {
        Action::NoOp => {}
        Action::AdjustConfig(rate) => {
            profile.adjust_level(rate);
            let conserve_rate = 0.6 * rate;
            block_send(src_ctrl, AdaptSignal::ToRate(conserve_rate));
            info!("adjust config {:?}, rate: {}", action, conserve_rate);
        }
        Action::AdvanceConfig => {
            profile.advance_level();
            block_send(src_ctrl, AdaptSignal::DecreaseDegradation);
            info!("advance config {:?}", action);
        }
        Action::StartProbe => {
            let delta = profile.next_rate_delta().expect("Must not at max config");
            block_send(src_ctrl, AdaptSignal::StartProbe(delta));
            info!("start probing for {:?}", delta);
        }
        Action::IncreaseProbePace => {
            block_send(src_ctrl, AdaptSignal::IncreaseProbePace);
            info!("increase probe pace");
        }
        Action::StopProbe => {
            block_send(src_ctrl, AdaptSignal::StopProbe);
            info!("increase probe pace");
        }
    }
}
