//! Adapatation algorithm implementation (described as in Figure 6).

/// Signal
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    /// QueueCongest signal carries the outgoing rate and the estimated latency.
    QueueCongest(f64, f64),

    /// Queue is empty, try to be aggressive.
    QueueEmpty,

    ProbeDone,
    ConfigMax,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    NoOp,
    AdvanceConfig,

    /// When the action is `AdjustConfig`, we inform the estimated outgoing rate
    AdjustConfig(f64),

    StartProbe,
    IncreaseProbePace,
    StopProbe,
}

#[derive(Debug, Clone, Copy)]
/// States of the rate adaptation algorithm.
enum State {
    Startup,
    Degrade,
    Steady,
    Probe,
}

pub struct Adaptation {
    state: State,
}

impl Default for Adaptation {
    fn default() -> Adaptation {
        Adaptation { state: State::Startup }
    }
}

impl Adaptation {
    pub fn transit(&mut self, signal: Signal, max_config: bool) -> Action {
        info!(
            "state: {:?}, signal: {:?}, max?: {}",
            self.state,
            signal,
            max_config
        );
        let action = match (self.state, signal, max_config) {
            (State::Startup, Signal::QueueEmpty, false) => {
                // transition 1
                Action::AdvanceConfig
            }
            (State::Startup, Signal::QueueEmpty, true) => {
                // transition 2, queue is empty and config at max
                self.state = State::Steady;
                Action::NoOp
            }
            (State::Startup, Signal::QueueCongest(rate, _latency), _) => {
                // transition 3
                self.state = State::Degrade;
                Action::AdjustConfig(rate)
            }
            (State::Degrade, Signal::QueueCongest(rate, _latency), _) => {
                // transition 4
                self.state = State::Degrade;
                Action::AdjustConfig(rate)
            }
            (State::Degrade, Signal::QueueEmpty, _) => {
                // transition 5
                self.state = State::Steady;
                Action::NoOp
            }
            (State::Steady, Signal::QueueCongest(rate, _latency), _) => {
                // transition 6
                self.state = State::Degrade;
                Action::AdjustConfig(rate)
            }
            (State::Steady, Signal::QueueEmpty, false) => {
                // transition 7
                self.state = State::Probe;
                Action::StartProbe
            }
            (State::Probe, Signal::QueueCongest(_rate, _latency), _) => {
                // transtion 8
                self.state = State::Steady;
                Action::StopProbe
            }
            (State::Probe, Signal::ProbeDone, _) => {
                // transition 9
                self.state = State::Steady;
                Action::AdvanceConfig
            }
            (State::Probe, Signal::QueueEmpty, _) => {
                // transition 10
                Action::IncreaseProbePace
            }
            (State::Steady, Signal::QueueEmpty, true) => {
                // The right state to stay in for as long as possible
                Action::NoOp
            }
            _ => {
                error!("Unhandled state {:?} and signal {:?}", self.state, signal);
                unimplemented!{}
            }
        };
        info!("state: {:?}, action: {:?}", self.state, action);
        action
    }
}
