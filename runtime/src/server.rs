//! The main entrance for server functionality.

use super::{AsCodec, AsDatumType};
use chrono;
use chrono::{DateTime, TimeZone};
use futures::{Future, Stream};
use interval;
use io::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
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

fn handle_connection(socket: TcpStream, handle: &Handle) -> Result<()> {
    let transport = socket.framed(AsCodec::default());
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    let timer = tokio_timer::Timer::default();
    let (ticks, tick_stopper) = interval::new(timer, Duration::from_millis(1000));

    let estimate_throughput = ticks.for_each(move |_| {
        // in each tick, measure bandwidth
        let bytes = count_clone.swap(0, Ordering::SeqCst);
        let kbps = bytes as f64 * 8.0 / 1000.0;
        info!("bw: {} kbps", kbps);
        Ok(())
    });

    // Spawn a new task dedicated to measure bandwidth
    handle.spawn(estimate_throughput.map_err(|_| ()));

    let (_transport_write, transport_read) = transport.split();
    let process_connection = transport_read
        .for_each(move |as_datum| {
            let size = as_datum.len() as usize;
            count.fetch_add(size, Ordering::SeqCst);
            match as_datum.datum_type() {
                AsDatumType::Live(level) => {
                    let now = chrono::Utc::now();
                    let latency = time_diff_in_ms(now, as_datum.ts);
                    info!(
                        "level: {}, latency: {:.1} ms, size: {}",
                        level,
                        latency,
                        size
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
