//! Error types for AWStream.

/// Creates the Error, ErrorKind, ResultExt, and Result types
error_chain!{
    errors {
    }

    foreign_links {
        Io(::std::io::Error);
    }
}
