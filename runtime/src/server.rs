//! The main entrance for server functionality.

use super::{AsCodec, AsDatum, AsDatumType, ReceiverReport};
use super::analytics::VideoAnalytics;
use super::bw_monitor::{BwMonitor, LatencyMonitor};
use super::setting::Setting;
use super::utils::StreamingStat;
use chrono;
use chrono::{DateTime, TimeZone, Utc};
use errors::*;
use futures::{Future, Sink, Stream};
use interval;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Handle};
use tokio_io::AsyncRead;
use tokio_timer;

fn time_diff_in_ms<Tz: TimeZone>(a: DateTime<Tz>, b: DateTime<Tz>) -> f64 {
    (a.timestamp() as f64 - b.timestamp() as f64) * 1000.0 +
        (a.timestamp_subsec_millis() as f64 - b.timestamp_subsec_millis() as f64)
}

/// Run the server. The server listens for new connections, parses input, and
/// prints performance statistics (latency, accuracy, etc).
///
/// The function will block until the server is shutdown.
pub fn server(setting: Setting) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let addr = ([0, 0, 0, 0], setting.port).into();
    let listener = TcpListener::bind(&addr, &handle).unwrap();

    // Accept all incoming sockets
    let server = listener.incoming().for_each(move |(socket, addr)| {
        let analytics = VideoAnalytics::new(&setting.profile_path, &setting.stat_path);
        handle_conn(socket, addr, analytics, &handle)
    });

    // Open listener
    core.run(server).unwrap();
}

/// The main server logic that handles a particular socket.
fn handle_conn(
    socket: TcpStream,
    addr: SocketAddr,
    analytics: VideoAnalytics,
    handle: &Handle,
) -> io::Result<()> {
    info!("new connection from {}", addr);

    let transport = socket.framed(AsCodec::default());
    let (transport_write, transport_read) = transport.split();

    let mut goodput = BwMonitor::new();
    let mut throughput = BwMonitor::new();
    let mut latency_mon = LatencyMonitor::new();
    let mut reporter = Reporter::new(
        transport_write,
        goodput.clone(),
        throughput.clone(),
        latency_mon.clone(),
        analytics.clone(),
    );

    let timer = tokio_timer::Timer::default();
    let (ticks, tick_stopper) = interval::new(timer, Duration::from_millis(1000));

    let errmsg = "fail to update statistics";

    let estimate_throughput = ticks.for_each(move |_| {
        // in each tick, measure bandwidth
        goodput.update(1000).expect(&errmsg);
        throughput.update(1000).expect(&errmsg);;
        latency_mon.update().expect(&errmsg);;
        info!(
            "client {}\tgoodput {} kbps\tthroughput {} kbps\tlatency {:.3} ms\taccuracy {:.4}",
            addr,
            goodput.rate().unwrap(),
            throughput.rate().unwrap(),
            latency_mon.rate().unwrap(),
            analytics.accuracy().unwrap()
        );
        Ok(())
    });

    // Spawn a new task dedicated to measure bandwidth
    handle.spawn(estimate_throughput.map_err(|_| ()));

    let process_connection = transport_read
        .for_each(move |as_datum| {
            let size = as_datum.len() as usize;
            reporter.throughput.add(size).expect(&errmsg);;
            match as_datum.datum_type() {
                AsDatumType::Live(level, frame_num) => {
                    let size = as_datum.len() as usize;
                    reporter.goodput.add(size).expect(&errmsg);
                    reporter.report(level, frame_num, as_datum)?
                }
                AsDatumType::Dummy => {}
                AsDatumType::LatencyProbe => {
                    let now = chrono::Utc::now();
                    let latency = time_diff_in_ms(now, as_datum.ts);
                    reporter.update_net_latency(latency);
                }
                _ => {}
            }
            Ok(())
        })
        .map_err(|_| ());

    // Spawn a new task dedicated to processing the connection
    handle.spawn(process_connection.and_then(|_| {
        tick_stopper.send(()).expect("failed to send");
        Ok(())
    }));
    Ok(())
}

struct Reporter<T: Sink<SinkItem = AsDatum, SinkError = Error>> {
    last_report_time: DateTime<Utc>,
    net_latency: StreamingStat,
    app_latency: StreamingStat,
    reporter: T,

    goodput: BwMonitor,
    throughput: BwMonitor,
    latency: LatencyMonitor,

    analytics: VideoAnalytics,
}

impl<T: Sink<SinkItem = AsDatum, SinkError = Error>> Reporter<T> {
    pub fn new(
        reporter: T,
        goodput: BwMonitor,
        throughput: BwMonitor,
        latency: LatencyMonitor,
        analytics: VideoAnalytics,
    ) -> Self {
        Reporter {
            last_report_time: chrono::Utc::now(),
            net_latency: StreamingStat::new(::std::f64::INFINITY, 10),
            app_latency: StreamingStat::new(::std::f64::INFINITY, 10),
            reporter: reporter,
            goodput: goodput,
            throughput: throughput,
            latency: latency,
            analytics: analytics,
        }
    }

    pub fn update_app_latency(&mut self, latency: f64) {
        self.app_latency.add(latency);
    }

    pub fn update_net_latency(&mut self, latency: f64) {
        self.net_latency.add(latency);
    }

    pub fn update_latency(&mut self, latency: f64) {
        self.latency.add(latency).expect(
            &"failed to update latency",
        );
    }

    /// report is called whenever we receive a new datum
    pub fn report(&mut self, level: usize, frame_num: usize, datum: AsDatum) -> Result<()> {
        let ts = datum.ts;
        let now = chrono::Utc::now();
        let latency = time_diff_in_ms(now, ts);
        self.update_latency(latency);
        self.update_app_latency(latency);
        self.analytics.add(frame_num, level)?;
        trace!(
            "level: {}, latency: {:.1}, size: {}",
            level,
            latency,
            datum.len()
        );

        if self.latency_is_high(latency, &datum) {
            let time_since_last_report = time_diff_in_ms(now, self.last_report_time);
            if time_since_last_report > 500.0 {
                self.last_report_time = now;
                let report = ReceiverReport::new(
                    latency,
                    self.goodput.rate().unwrap(),
                    self.throughput.rate().unwrap(),
                );
                trace!("report {:?}", report);
                let datum = AsDatum::ack(report)?;
                self.reporter.start_send(datum)?;
                self.reporter.poll_complete()?;
            }
        }
        Ok(())
    }

    #[inline]
    fn latency_is_high(&self, current_latency: f64, datum: &AsDatum) -> bool {
        // Build a latency model: expected = min_net + size / rate + noise
        let net_delay = self.net_latency.min();
        let tx_delay = datum.len() as f64 / self.goodput.rate().unwrap();
        let ideal = net_delay + tx_delay;

        let expected = match ideal as u64 {
            0...100 => 10.0 * ideal,
            100...200 => 5.0 * ideal,
            200...300 => 4.0 * ideal,
            300...500 => 3.0 * ideal,
            _ => 1.5 * ideal,
        };

        current_latency > expected
    }
}
