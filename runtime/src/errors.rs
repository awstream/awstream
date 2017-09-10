//! Error types for AWStream.

/// Creates the Error, ErrorKind, ResultExt, and Result types
error_chain!{
    errors {
        SourceData {
            description("error in generating source data")
        }
        RemotePeer {
            description("error in receiving reports from peer")
        }
        ControlPlane {
            description("error in control plane")
        }
        DataPlane {
            description("error in data plane communication")
        }
        ReplyChannel {
            description("error in replying to client")
        }
        EncodeError {
            description("error in encoding the data")
        }
        DecodeError {
            description("error in decoding the data")
        }
        SyncPoisonError(t: String) {
        }
    }

    foreign_links {
        Io(::std::io::Error);
        Timer(::tokio_timer::TimerError);
        Bincode(::bincode::Error);
    }
}

impl<T> From<::std::sync::PoisonError<T>> for Error {
    fn from(err: ::std::sync::PoisonError<T>) -> Self {
        use std::error::Error;

        Self::from_kind(ErrorKind::SyncPoisonError(err.description().to_string()))
    }
}
