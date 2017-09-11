//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.

use super::{Adapt, AdaptAction, AsCodec, ReceiverReport};
use super::adaptation::{Action, Adaptation, Signal};
use super::controller::Monitor;
use super::errors::*;
use super::profile::SimpleProfile;
use super::setting::Setting;
use super::socket::{FramedRead, Socket};
use super::source::TimerSource;
use super::video::VideoSource;
use futures::{Future, Sink, Stream};

use futures::sync::mpsc::UnboundedSender;
use futures_cpupool::CpuPool;
use std::net::SocketAddr;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;

const PROBE_EXTRA: f64 = 1.05;

fn connect(server: &str, port: u16, core: &mut Core) -> Result<TcpStream> {
    let handle = core.handle();
    let ip = server.parse().unwrap();
    let address = SocketAddr::new(ip, port);

    let work = TcpStream::connect(&address, &handle);
    let tcp = core.run(work)?;
    // tcp.set_nodelay(true).expect("failed to set TCP NODELAY");
    // tcp.set_send_buffer_size(64 * 1_024).expect("failed to set send buffer");
    Ok(tcp)
}

/// Run client
pub fn run(setting: Setting) -> Result<()> {
    let pool = CpuPool::new_num_cpus();

    // Setting up the reactor core
    let mut core = Core::new().unwrap();

    // Creates the TCP connection (this is synchronous!)
    let tcp = connect(&setting.server, setting.port, &mut core)?;
    info!("conected to server: {}:{}", setting.server, setting.port);

    let video_source = VideoSource::new(setting.source_path, setting.profile_path);
    let mut profile = video_source.simple_profile();

    /////////////////////////////////////////////////////////////////
    //
    // Data Plane
    //
    /////////////////////////////////////////////////////////////////

    // 1. Creates source
    let handle = core.handle();
    let (src_ctrl, src_data, src_stat) = TimerSource::spawn(video_source, handle);

    // 2. Creates sink (socket)
    let (tcp_read, tcp_write) = tcp.split();
    let (socket, out_bytes) = Socket::new(tcp_write);

    // 3. Forward all source data to socket
    let s = src_data.map_err(|_| Error::from_kind(ErrorKind::SourceData));
    let socket_work = socket.send_all(s).map(|_| ()).map_err(|_| ());

    let data_plane = pool.spawn(socket_work);
    core.handle().spawn(data_plane);

    //////////////////////////////////////////////////////////////////
    //
    //  Control Plane
    //
    //////////////////////////////////////////////////////////////////
    let mut adaptation = Adaptation::default();

    let remote = FramedRead::new(tcp_read, AsCodec::default())
        .map(|as_datum| {
            let errmsg = "failed to parse mem into report";
            let report = ReceiverReport::from_mem(&as_datum.mem).expect(&errmsg);
            Signal::RemoteCongest(report.throughput, report.latency)
        })
        .map_err(|_| Error::from_kind(ErrorKind::RemotePeer));

    let (src_tx, src_rx) = src_ctrl;
    let monitor = Monitor::new(src_stat, out_bytes).skip(1);
    let probing = src_rx.map_err(|_| Error::from_kind(ErrorKind::RemotePeer));

    let control_plane = monitor
        .select(probing)
        .select(remote)
        .for_each(move |signal| {
            core_adapt(signal, &mut adaptation, &mut profile, src_tx.clone());
            Ok(())
        })
        .map_err(|_| Error::from_kind(ErrorKind::ControlPlane));

    let control_plane = pool.spawn(control_plane);
    core.run(control_plane)?;

    Ok(())
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
            let level = profile.adjust_level(rate);
            block_send(src_ctrl, AdaptAction::ToRate(rate));
            info!("adjust config, level: {:?}, rate: {}", level, rate);
        }
        Action::AdvanceConfig => {
            let level = profile.advance_level();
            block_send(src_ctrl, AdaptAction::DecreaseDegradation);
            info!("advance config to {:?}", level);
        }
        Action::StartProbe => {
            let delta = profile.next_rate_delta().expect("Must not at max config");
            let target = PROBE_EXTRA * delta; // probe more space than needed
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
