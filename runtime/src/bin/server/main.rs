extern crate awstream;
extern crate env_logger;
extern crate chrono;
extern crate log;

use awstream::*;
use std::{env, str};

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

    let setting = Setting::init("Setting.toml").unwrap();
    server::server(setting);
}
