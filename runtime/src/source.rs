use super::AsDatum;
use futures::{Future, Stream};
use futures::sync::mpsc::{UnboundedReceiver, unbounded};
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
