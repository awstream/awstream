//! Error types for AWStream.

use futures::sync::mpsc::SendError;
use std::any::Any;

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
    }

    foreign_links {
        Io(::std::io::Error);
        Timer(::tokio_timer::TimerError);
    }
}

impl<T: Any> From<SendError<T>> for Error {
    fn from(_err: SendError<T>) -> Self {
        Self::from_kind(ErrorKind::DataPlane)
    }
}
