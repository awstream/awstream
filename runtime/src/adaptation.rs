//! Adapatation algorithm implementation (described as in Figure 6).

/// Signal
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    /// QueueCongest signal carries the outgoing rate and the estimated latency.
    QueueCongest(f64, f64),

    /// Queue is empty, try to be aggressive.
    QueueEmpty,

    /// Congestion signal from the remote.
    RemoteCongest(f64, f64),

    /// Probe done
    ProbeDone,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    NoOp,
    AdvanceConfig,

    /// When the action is `AdjustConfig`, we inform the estimated outgoing rate
    AdjustConfig(f64),

    /// Start the probe with a target bandwidth (in kbps)
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
    steady_count: usize,
    startup_congest: usize,
}

impl Default for Adaptation {
    fn default() -> Adaptation {
        Adaptation {
            state: State::Startup,
            steady_count: 0,
            startup_congest: 0,
        }
    }
}

impl Adaptation {
    /// Allow (transit) congestion during the startup phase as TCP is adjusting
    const STARTUP_CONGEST_ENOUGH: usize = 3;

    /// Only start probing if we are steady enough (that is, enough Q_E).
    const STEADY_ENOUGH: usize = 3;

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
            (State::Startup, Signal::QueueCongest(rate, _latency), _) |
            (State::Startup, Signal::RemoteCongest(rate, _latency), _) => {
                // transition 3
                // transition 7
                if self.startup_congest > Adaptation::STARTUP_CONGEST_ENOUGH {
                    self.startup_congest = 0;
                    self.state = State::Degrade;
                    Action::AdjustConfig(rate)
                } else {
                    self.startup_congest += 1;
                    Action::NoOp
                }
            }
            (State::Degrade, Signal::QueueCongest(rate, _latency), _) |
            (State::Degrade, Signal::RemoteCongest(rate, _latency), _) => {
                // transition 4
                self.state = State::Degrade;
                Action::AdjustConfig(rate)
            }
            (State::Degrade, Signal::QueueEmpty, _) => {
                // transition 5
                self.state = State::Steady;
                Action::NoOp
            }
            (State::Steady, Signal::QueueCongest(rate, _latency), _) |
            (State::Steady, Signal::RemoteCongest(rate, _latency), _) => {
                // transition 6
                self.steady_count = 0;
                self.state = State::Degrade;
                Action::AdjustConfig(rate)
            }
            (State::Steady, Signal::QueueEmpty, false) => {
                // transition 7
                if self.steady_count > Adaptation::STEADY_ENOUGH {
                    self.steady_count = 0;
                    self.state = State::Probe;
                    Action::StartProbe
                } else {
                    self.steady_count += 1;
                    Action::NoOp
                }
            }
            (State::Probe, Signal::QueueCongest(_rate, _latency), _) |
            (State::Probe, Signal::RemoteCongest(_rate, _latency), _) => {
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
