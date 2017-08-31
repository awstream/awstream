//! Using a transport directly
//!
//! This example illustrates a use case where the protocol isn't request /
//! response oriented. In this case, the connection is established, and "log"
//! entries are streamed to the remote.
//!
//! Given that the use case is not request / response oriented, it doesn't make
//! sense to use `tokio-proto`. Instead, we use the transport directly.

extern crate awstream;
extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate bytes;
extern crate env_logger;
extern crate chrono;
extern crate log;

use awstream::*;
use std::{env, str, thread};
use std::time::Duration;

pub fn main() {
    let format = |record: &log::LogRecord| {
        let t = chrono::Utc::now();
        format!(
            "{} {}:{}: {}",
            t.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            record.level(),
            record.location().module_path(),
            record.args()
        )
    };

    let mut builder = env_logger::LogBuilder::new();
    builder.format(format);
    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.init().unwrap();

    // Run the server in a dedicated thread
    thread::spawn(|| server::server());

    // Wait a moment for the server to start...
    thread::sleep(Duration::from_millis(100));

    // Client runs
    client::run();
}
