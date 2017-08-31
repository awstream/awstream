//! The main entrance for server functionality.

use AsCodec;
use chrono;
use futures::{Future, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;

/// Run the server. The server listens for new connections, parses input, and
/// prints performance statistics (latency, accuracy, etc).
///
/// The function will block until the server is shutdown.
pub fn server() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let remote_addr = "0.0.0.0:8888".parse().unwrap();

    let listener = TcpListener::bind(&remote_addr, &handle).unwrap();

    // Accept all incoming sockets
    let server = listener.incoming().for_each(move |(socket, _)| {
        let transport = socket.framed(AsCodec::default());

        let process_connection = transport.for_each(|as_datum| {
            let now = chrono::Utc::now().timestamp();
            info!("level: {}, latency: {}", as_datum.level, now - as_datum.ts);
            Ok(())
        });

        // Spawn a new task dedicated to processing the connection
        handle.spawn(process_connection.map_err(|_| ()));
        Ok(())
    });

    // Open listener
    core.run(server).unwrap();
}
