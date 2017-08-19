//! The client manages all components: `Source`, `Monitor`, `Socket` using an
//! event loop (`tokio_core::Core`). The loop selects the next available event
//! and reacts accordingly.
//!
//! The event is one of the following:
//!  * `Source` timeouts and sends an `AsDatum` item
//!  * `Socket` finishes sending the previous item
//!  * `AC` timeous and returns a congestion status

pub struct Client;
