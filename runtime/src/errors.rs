//! Error types for AWStream.

/// Creates the Error, ErrorKind, ResultExt, and Result types
error_chain!{
    errors {
        SourceDataErr {
            description("error in generating source data")
        }
        RemotePeer {
            description("error in receiving reports from peer")
        }
        ControlPlane {
            description("error in control plane")
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
