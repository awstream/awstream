//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.

use super::{Adapt, AdaptAction, AsCodec, ReceiverReport};
use super::adaptation::{Action, Adaptation, Signal};
use super::controller::Monitor;
use super::profile::SimpleProfile;
use super::setting::Setting;
use super::socket::{FramedRead, Socket};
use super::source::TimerSource;
use super::video::VideoSource;
use futures::{Future, Sink, Stream};
use futures::sync::mpsc::UnboundedSender;
use io;
use std::net::SocketAddr;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;

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
    // tcp.set_nodelay(true).expect("failed to set TCP NODELAY");
    // tcp.set_send_buffer_size(64 * 1_024).expect("failed to set send buffer");

    let video_source = VideoSource::new(setting.source_path, setting.profile_path);
    let mut profile = video_source.simple_profile();

    // First we create source
    let handle = core.handle();
    let (src_ctrl, source, src_bytes, probe_done) = TimerSource::spawn(video_source, handle);

    // Then we create sink (socket)
    let (tcp_read, tcp_write) = tcp.split();
    let (socket, out_bytes) = Socket::new(tcp_write);

    // Next, we forward all source data to socket
    let s = source.map_err(|_| {
        io::Error::new(io::ErrorKind::BrokenPipe, "failed to receive")
    });
    let socket_work = socket.send_all(s).map(|_| ()).map_err(|_| ());
    core.handle().spawn(socket_work);

    // Lastly, we create adaptation
    let mut adaptation = Adaptation::default();

    //////////////////////////////////////////////////////////////////
    //
    //  Merge three different streams
    //
    //////////////////////////////////////////////////////////////////
    let remote = FramedRead::new(tcp_read, AsCodec::default())
        .map(|as_datum| {
            let report = ReceiverReport::from_mem(&as_datum.mem);
            Signal::RemoteCongest(report.throughput, report.latency)
        })
        .map_err(|_| ());

    let monitor = Monitor::new(src_bytes, out_bytes).skip(1);
    let probing = probe_done.map(|_| Signal::ProbeDone);

    let work = monitor
        .select(probing)
        .select(remote)
        .for_each(|signal| {
            core_adapt(signal, &mut adaptation, &mut profile, src_ctrl.clone());
            Ok(())
        })
        .map_err(|_| ());

    core.run(work).unwrap();
}

fn block_send<T>(tx: UnboundedSender<T>, item: T) {
    let errmsg = "failed to control source";
    tx.send(item).wait().expect(&errmsg);
}

fn core_adapt(
    signal: Signal,
    adaptation: &mut Adaptation,
    profile: &mut SimpleProfile,
    src_ctrl: UnboundedSender<AdaptAction>,
) {
    let action = adaptation.transit(signal, profile.is_max());
    match action {
        Action::NoOp => {}
        Action::AdjustConfig(rate) => {
            let conserve_rate = 0.9 * rate;
            let level = profile.adjust_level(conserve_rate);
            block_send(src_ctrl, AdaptAction::ToRate(conserve_rate));
            info!("adjust config, level: {:?}, rate: {}", level, conserve_rate);
        }
        Action::AdvanceConfig => {
            let level = profile.advance_level();
            block_send(src_ctrl, AdaptAction::DecreaseDegradation);
            info!("advance config to {:?}", level);
        }
        Action::StartProbe => {
            let delta = profile.next_rate_delta().expect("Must not at max config");
            let target = 1.05 * delta; // probe more space than strictly needed
            block_send(src_ctrl, AdaptAction::StartProbe(target));
            info!("start probing for {:?}", target);
        }
        Action::IncreaseProbePace => {
            block_send(src_ctrl, AdaptAction::IncreaseProbePace);
            info!("increase probe pace");
        }
        Action::StopProbe => {
            block_send(src_ctrl, AdaptAction::StopProbe);
            info!("stop probe pace");
        }
    }
}
