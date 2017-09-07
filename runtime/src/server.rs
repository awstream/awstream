//! The main entrance for server functionality.

use super::{AsCodec, AsDatum, AsDatumType, ReceiverReport};
use super::bw_monitor::BwMonitor;
use super::utils::StreamingStat;
use chrono;
use chrono::{DateTime, TimeZone, Utc};
use errors::*;
use futures::{Future, Sink, Stream};
use interval;
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
pub fn server(port: u16) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let addr = ([0, 0, 0, 0], port).into();
    let listener = TcpListener::bind(&addr, &handle).unwrap();

    // Accept all incoming sockets
    let server = listener.incoming().for_each(move |(socket, _addr)| {
        handle_connection(socket, &handle)
    });

    // Open listener
    core.run(server).unwrap();
}

/// The main server logic that handles a particular socket.
fn handle_connection(socket: TcpStream, handle: &Handle) -> ::std::io::Result<()> {
    let transport = socket.framed(AsCodec::default());
    let (transport_write, transport_read) = transport.split();

    let mut goodput = BwMonitor::new();
    let mut throughput = BwMonitor::new();
    let mut reporter = Reporter::new(transport_write, goodput.clone(), throughput.clone());

    let timer = tokio_timer::Timer::default();
    let (ticks, tick_stopper) = interval::new(timer, Duration::from_millis(1000));

    let estimate_throughput = ticks.for_each(move |_| {
        // in each tick, measure bandwidth
        goodput.update(1000);
        throughput.update(1000);
        info!(
            "goodput: {} kbps, throughput: {} kbps",
            goodput.rate(), throughput.rate(),
        );
        Ok(())
    });

    // Spawn a new task dedicated to measure bandwidth
    handle.spawn(estimate_throughput.map_err(|_| ()));

    let process_connection = transport_read
        .for_each(move |as_datum| {
            let size = as_datum.len() as usize;
            reporter.throughput.add(size);
            match as_datum.datum_type() {
                AsDatumType::Live(level) => {
                    let size = as_datum.len() as usize;
                    reporter.goodput.add(size);
                    reporter.report(level, as_datum)?
                }
                AsDatumType::Dummy => {}
                AsDatumType::LatencyProbe => {
                    let now = chrono::Utc::now();
                    let latency = time_diff_in_ms(now, as_datum.ts);
                    reporter.update_min_latency(latency);
                    info!(
                        "latency estimate: {}/{:.1}",
                        latency,
                        reporter.min_latency()
                    );
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
    min_latency: StreamingStat,
    reporter: T,

    goodput: BwMonitor,
    throughput: BwMonitor,
}

impl<T: Sink<SinkItem = AsDatum, SinkError = Error>> Reporter<T> {
    pub fn new(reporter: T, goodput: BwMonitor, throughput: BwMonitor) -> Self {
        Reporter {
            last_report_time: chrono::Utc::now(),
            min_latency: StreamingStat::new(::std::f64::INFINITY, 10),
            reporter: reporter,
            goodput: goodput,
            throughput: throughput,
        }
    }

    pub fn update_min_latency(&mut self, latency: f64) {
        self.min_latency.add(latency);
    }

    pub fn min_latency(&self) -> f64 {
        self.min_latency.min()
    }
    pub fn report(&mut self, level: usize, datum: AsDatum) -> Result<()> {
        let ts = datum.ts;
        let now = chrono::Utc::now();
        let latency = time_diff_in_ms(now, ts);
        info!(
            "level: {}, latency: {:.1}, size: {}",
            level,
            latency,
            datum.len()
        );
        if latency > 10.0 * self.min_latency.min() && latency > 10.0 {
            let time_since_last_report = time_diff_in_ms(now, self.last_report_time);
            if time_since_last_report > 500.0 {
                self.last_report_time = now;
                info!("reporting latency spikes {}", self.min_latency.min());
                let report =
                    ReceiverReport::new(latency, self.goodput.rate(), self.throughput.rate());

                let datum = AsDatum::ack(report);
                self.reporter.start_send(datum)?;
                self.reporter.poll_complete()?;
            }
        }
        Ok(())
    }
}
