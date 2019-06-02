//! This crate wraps `gstreamer` and `opencv` to simplify the interface of
//! loading videos. Currenlty it only supports loading image but adding video
//! files is easy (TODO).
//!
//! There are two types of data that can be loaded: frame (`cv::Mat`) and x264
//! encoded bytes. Both interface will return a Receiver<T> that applications
//! can use.
//!
//! Internally there will be multiple threads running (two for `load_frame`;
//! four for `load_x264`).
//!
//! Illustrated in a diagram as below:
//!
//! ```text
//!           Interface 1                                   Interface 2
//!            (tx, rx)
//! schedule_recv => frame_loader => gstreamer => x264_loader => APP
//! ```
//!
//! We also support directly loading x264 encoded stream from file.

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
extern crate cv;
extern crate gst;
extern crate schedule_recv;
extern crate csv;

pub mod loader;
mod pipeline;

mod errors {
    use gst;
    use csv;
    error_chain!{
        foreign_links {
            Io(::std::io::Error);
            Recv(::std::sync::mpsc::RecvError);
            Csv(csv::Error);
        }

        errors {
            Gst(t: String) {
                description("gstreamer internal")
                display("gstreamer internal error {}", t)
            }
            EndStream {
                description("end of stream")
                display("end of stream")
            }
        }
    }

    impl From<gst::Error> for Error {
        fn from(err: gst::Error) -> Error {
            Error::from_kind(ErrorKind::Gst(err.message()))
        }
    }
}

fn skip_to_fps(skip: usize) -> f64 {
    (30.0 / (skip as f64 + 1.0) * 10.0).round() / 10.0
}
