use super::{Adapt, AdaptSignal, Experiment};
use super::AsDatum;
use futures::Stream;
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio_core::reactor::Handle;
use tokio_timer;

type AdaptControl = UnboundedSender<AdaptSignal>;
type DataChannel = UnboundedReceiver<AsDatum>;

pub type SourceCtrl = (AdaptControl, DataChannel, Arc<AtomicUsize>);

pub struct TimerSource;

enum Incoming {
    Timer,
    Adapt(AdaptSignal),
}

impl TimerSource {
    pub fn spawn<As: Adapt + Experiment + 'static>(
        mut source: As,
        period: Duration,
        handle: Handle,
    ) -> SourceCtrl {
        let timer = tokio_timer::wheel()
            .tick_duration(Duration::from_millis(1))
            .build()
            .interval(period)
            .map_err(|_e| ())
            .map(|_e| Incoming::Timer);

        let (adapt_tx, adapt_rx) = unbounded();
        let adapter = adapt_rx.map(|level| Incoming::Adapt(level));

        let (data_tx, data_rx) = unbounded();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let work = timer.select(adapter).for_each(
            move |incoming| match incoming {
                Incoming::Timer => {
                    let size = source.next_datum();
                    if size == 0 {
                        return Ok(());
                    }

                    let data_to_send = AsDatum::new(source.current_level(), vec![0; size]);
                    info!("add new data {}", data_to_send.len());
                    counter_clone.clone().fetch_add(
                        data_to_send.len(),
                        Ordering::SeqCst,
                    );
                    data_tx.clone().send(data_to_send).map(|_| ()).map_err(
                        |_| (),
                    )
                }
                Incoming::Adapt(AdaptSignal::ToRate(rate)) => {
                    source.adapt(rate);
                    Ok(())
                }
                Incoming::Adapt(AdaptSignal::DecreaseDegradation) => {
                    source.dec_degradation();
                    Ok(())
                }
            },
        );
        handle.spawn(work);

        (adapt_tx, data_rx, counter.clone())
    }
}
