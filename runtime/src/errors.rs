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
    }

    foreign_links {
        Io(::std::io::Error);
        Timer(::tokio_timer::TimerError);
    }
}
