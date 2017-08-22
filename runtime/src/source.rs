use super::AsDatum;
use csv;
use futures::{Future, Stream};
use futures::sync::mpsc::{UnboundedReceiver, unbounded};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio_core::reactor::Handle;
use tokio_timer;

pub struct TimerSource;

impl TimerSource {
    pub fn spawn(
        period: Duration,
        handle: Handle,
    ) -> (UnboundedReceiver<AsDatum>, Arc<AtomicUsize>) {
        let timer = tokio_timer::wheel()
            .tick_duration(Duration::from_millis(50))
            .build()
            .interval(period);

        let (tx, rx) = unbounded();
        let counter = Arc::new(AtomicUsize::new(0));
        let ret = (rx, counter.clone());

        let source = timer
            .map_err(|_| ())
            .for_each(move |_| {
                let data_to_send = AsDatum::new(vec![0; 1_024_000]);
                info!("add new data {}", data_to_send.len());
                counter.fetch_add(data_to_send.len(), Ordering::SeqCst);
                tx.clone().send(data_to_send).map(|_| ()).map_err(|_| ())
            })
            .map_err(|_| ());

        handle.spawn(source);
        ret
    }
}

#[derive(Deserialize)]
struct Record {
    width: usize,
    skip: usize,
    quant: usize,
    frame: usize,
    bytes: usize,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct VideoConfig {
    pub width: usize,
    pub skip: usize,
    pub quant: usize,
}

impl VideoConfig {
    pub fn new(w: usize, s: usize, q: usize) -> Self {
        VideoConfig {
            width: w,
            skip: s,
            quant: q,
        }
    }
}

impl ::std::fmt::Display for VideoConfig {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}x{}x{}", self.width, self.skip, self.quant)
    }
}

pub struct VideoSource {
    map: BTreeMap<(VideoConfig, usize), usize>,
    frame: usize,
    num: usize,
    config: VideoConfig,
}

impl VideoSource {
    pub fn new<P>(path: P, init_config: VideoConfig) -> VideoSource
    where
        P: AsRef<Path>,
    {
        let errmsg = format!("no source file {:?}", path.as_ref());
        let mut rdr = csv::Reader::from_path(path).expect(&errmsg);
        let mut map = BTreeMap::new();
        let mut num = 0;
        for record in rdr.deserialize() {
            let record: (VideoConfig, usize, usize) = record.expect("failed to parse the record");
            map.insert((record.0, record.1), record.2);
            num = ::std::cmp::max(num, record.1);
        }
        VideoSource {
            map: map,
            frame: 0,
            num: num,
            config: init_config,
        }
    }

    pub fn set_config(&mut self, config: VideoConfig) {
        self.config = config;
    }

    pub fn next_frame(&mut self) -> usize {
        let frame_size = self.map.get(&(self.config, self.frame)).expect(&format!(
            "Source file corrupted. Failed to find frame size for {}@{}",
            self.config,
            self.frame
        ));
        self.frame += 1;
        if self.frame >= self.num {
            self.frame = 0;
        }
        *frame_size
    }
}
