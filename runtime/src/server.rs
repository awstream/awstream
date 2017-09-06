//! The main entrance for server functionality.

use super::{AsCodec, AsDatum, AsDatumType, ReceiverReport};
use super::bw_monitor::BwMonitor;
use super::utils::StreamingStat;
use chrono;
use chrono::{DateTime, TimeZone};
use futures::{Future, Sink, Stream};
use interval;
use io::Result;
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
fn handle_connection(socket: TcpStream, handle: &Handle) -> Result<()> {
    let transport = socket.framed(AsCodec::default());

    let mut goodput = BwMonitor::new();
    let mut goodput2 = goodput.clone();
    let mut throughput = BwMonitor::new();
    let mut throughput2 = throughput.clone();

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

    let mut min_latency = StreamingStat::new(::std::f64::INFINITY, 10);

    let (mut transport_write, transport_read) = transport.split();
    let process_connection = transport_read
        .for_each(move |as_datum| {
            match as_datum.datum_type() {
                AsDatumType::Live(level) => {
                    let size = as_datum.len() as usize;
                    goodput2.add(size);

                    let now = chrono::Utc::now();
                    let latency = time_diff_in_ms(now, as_datum.ts);

                    if latency > 20.0 * min_latency.min() && latency > 10.0 {
                        info!("reporting latency spikes {}", min_latency.min());
                        let report =
                            ReceiverReport::new(latency, goodput2.rate(), throughput2.rate());
                        transport_write.start_send(AsDatum::ack(report)).expect(
                            "failed to write back",
                        );
                        transport_write.poll_complete().expect(
                            "failed to write back",
                        );
                    }
                    info!(
                        "level: {}, latency: {:.1} ms, size: {}",
                        level,
                        latency,
                        size
                    );
                }
                AsDatumType::Dummy => {}
                AsDatumType::LatencyProbe => {
                    let now = chrono::Utc::now();
                    let latency = time_diff_in_ms(now, as_datum.ts);
                    min_latency.add(latency);
                    info!("latency estimate: {}/{:.1}", latency, min_latency.min());
                }
                _ => {}
            }
            let size = as_datum.len() as usize;
            throughput2.add(size);
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
