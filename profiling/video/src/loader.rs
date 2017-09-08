use std::path::Path;
use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use std::ptr::copy;
use std::io::Read;

use csv;
use cv::imgcodecs::ImreadModes::ImreadColor;
use cv::imgproc::InterpolationFlag;
use cv;
use gst;
use pipeline::{create_caps, gst_main_loop};
use schedule_recv;

use super::errors::*;
use super::skip_to_fps;

pub struct LoaderConfig {
    pub path: String,
    pub ext: String,
    pub circular: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VideoConfig {
    pub width: usize,
    pub height: usize,
    pub skip: usize,
    pub quantizer: usize,
}

pub fn load_encoded(lc: LoaderConfig,
                    vc: VideoConfig)
                    -> Result<(Receiver<Vec<u8>>, LoaderHandle)> {
    let mut frame_num = 1;

    let (loader_handle, _loader_rx) = channel::<VideoConfig>();
    let (tx, rx) = channel();

    ::std::thread::spawn(move || {
        let path = lc.path;
        'outer: loop {
            let fps = skip_to_fps(vc.skip);
            let period = (1000.0 as f64 / fps).round() as u32;
            debug!("schedule_recv period {} ms", period);
            let tick = schedule_recv::periodic_ms(period);
            'inner: loop {
                // Load in a synchronous way.
                tick.recv().expect("video_loader: failed in ticking");

                let filename = format!("{}/{:06}", &path, frame_num);
                trace!("tick: {}", filename);
                frame_num += vc.skip + 1;

                if ::std::fs::metadata(&filename).is_ok() {
                    let mut f = ::std::fs::File::open(filename)
                        .expect("video_loader: failed in open file");
                    let mut buf = Vec::new();
                    f.read_to_end(&mut buf).expect("video_loader: failed in read");
                    tx.send(buf).expect("video_loader: failed to send");
                } else {
                    if lc.circular {
                        frame_num = 1;
                    } else {
                        break 'outer;
                    }
                }
            }
        }
    });

    Ok((rx, loader_handle))
}


type SimulateSize = (usize, usize);

pub fn load_simulated(lc: LoaderConfig,
                      mut vc: VideoConfig)
                      -> Result<(Receiver<Vec<u8>>, LoaderHandle)> {
    let (loader_handle, loader_rx) = channel::<VideoConfig>();
    let (tx, rx) = channel();

    ::std::thread::spawn(move || {
        'outer: loop {
            let mut frame_num = 0;

            // Prepare tick based on skip
            let fps = skip_to_fps(vc.skip);
            let period = (1000.0 as f64 / fps).round() as u32;
            debug!("schedule_recv period {} ms", period);
            let tick = schedule_recv::periodic_ms(period);

            // Prepare file to read based on vc.width and vc.quantizer
            let simulation_filename = format!("{}/data/bw-{}x{}x{}.csv",
                                              lc.path,
                                              vc.width,
                                              vc.skip,
                                              vc.quantizer);
            debug!("use simulation file {}", simulation_filename);
            let mut rdr = csv::Reader::from_file(simulation_filename)
                .expect("failed to load file")
                .has_headers(false);
            let all_info = rdr.decode().collect::<csv::Result<Vec<SimulateSize>>>().unwrap();

            'inner: loop {
                // First we check if we have received new configuration. In an
                // update, break the inner loop (to update fps) and update the
                // simulation file.
                if let Ok(new_config) = loader_rx.try_recv() {
                    vc = new_config;
                    break 'inner;
                }

                // Load in a synchronous way.
                tick.recv().expect("video_loader: failed in ticking");
                trace!("tick");

                let size = {
                    if frame_num < all_info.len() {
                        all_info[frame_num].1
                    } else {
                        frame_num = 0;
                        all_info[frame_num].1
                    }
                };
                tx.send(vec![0; size]).expect("video_loader: failed to send");
                frame_num += 1;
            }
        }
    });

    Ok((rx, loader_handle))
}

pub fn load_frame(lc: LoaderConfig, vc: VideoConfig) -> Result<(Receiver<cv::Mat>, LoaderHandle)> {
    let (loader_handle, loader_rx) = channel();
    let (tx, rx) = channel();

    // Perform all tasks in a thread so that we can return the `rx`.
    thread::spawn(move || {
        let metadata = ::std::fs::metadata(&lc.path).expect("wrong path provided");
        let result = {
            if metadata.is_dir() {
                frame_loader(tx, loader_rx, lc, vc)
            } else {
                load_video_file(vc)
            }
        };

        // Handle errors
        match result {
            Ok(_) => {}
            Err(Error(ErrorKind::EndStream, _)) => {
                ::std::process::exit(0);
            }
            Err(ref e) => {
                println!("error: {}", e);

                for e in e.iter().skip(1) {
                    println!("caused by: {}", e);
                }

                // The backtrace is not always generated. Try to run this example
                // with `RUST_BACKTRACE=1`.
                if let Some(backtrace) = e.backtrace() {
                    println!("backtrace: {:?}", backtrace);
                }

                ::std::process::exit(1);
            }
        }
    });
    Ok((rx, loader_handle))
}

pub fn load_x264(lc: LoaderConfig,
                 config: VideoConfig)
                 -> Result<(Receiver<Vec<u8>>, LoaderHandle)> {
    let (frame_loader, frame_loader_handle) = load_frame(lc, config)?;
    let (loader, gstreamer_handle) = x264_encoder(frame_loader, config)?;

    let (tx, rx) = channel();
    thread::spawn(move || loop {
        match rx.recv() {
            Ok(vc) => {
                let _ = frame_loader_handle.send(vc);
                let _ = gstreamer_handle.send(vc);
            }
            Err(_) => warn!("The controller to video loader has been dropped!"),
        }
    });
    Ok((loader, tx))
}

fn load_video_file(_config: VideoConfig) -> Result<()> {
    unimplemented!();
}

fn frame_loader(tx: Sender<cv::Mat>,
                loader_rx: Receiver<VideoConfig>,
                lc: LoaderConfig,
                mut vc: VideoConfig)
                -> Result<()> {
    let mut frame_num = 1;
    let path = lc.path;
    let extension = lc.ext;

    'outer: loop {
        let fps = skip_to_fps(vc.skip);
        let period = (1000 as f64 / fps).round() as u32;
        debug!("schedule_recv period {} ms", period);
        let tick = schedule_recv::periodic_ms(period);
        'inner: loop {
            match loader_rx.try_recv() {
                Ok(new_config) => {
                    vc = new_config;
                    // Break the inner loop and start in the outer loop
                    break;
                }
                Err(_) => {}
            }

            // Load in a synchronous way.
            tick.recv()?;
            trace!("tick");

            let filename = format!("{}/{:06}.{}", &path, frame_num, extension);
            frame_num += vc.skip + 1;
            match cv_load_image(filename) {
                Ok(image) => tx.send(image).chain_err(|| "faild to send")?,
                Err(_) => {
                    if lc.circular {
                        frame_num = 1;
                    } else {
                        return Err(ErrorKind::EndStream.into());
                    }
                }
            }
        }
    }
}

fn cv_load_image<P: AsRef<Path>>(path: P) -> Result<cv::Mat> {
    trace!("cv_load_image from {:?}", path.as_ref());
    if path.as_ref().metadata().is_ok() {
        let frame = cv::Mat::from_path(&path, ImreadColor).unwrap();
        Ok(frame)
    } else {
        // Return Error?
        bail!("finished loading all images")
    }
}

pub type LoaderHandle = Sender<VideoConfig>;

fn x264_encoder(sched_rx: Receiver<cv::Mat>,
                config: VideoConfig)
                -> Result<(Receiver<Vec<u8>>, LoaderHandle)> {
    let (out_tx, out_rx) = channel();

    // loader_tx is returned so that applications can use it to control the
    // loader's behavior.
    let (loader_tx, loader_rx) = channel();

    // Create gstreamer loop
    let gst_handle = gst_main_loop(config)?;

    let (mut appsrc, appsink, mut buffer_pool) = gst_handle.to_tuple();

    let old_config = config.clone();

    // AppSrc thread
    thread::spawn(move || {
        let mut height = config.height;
        let mut width = config.width;
        let mut target_size = cv::Size2i::new(width as i32, height as i32);
        loop {
            match loader_rx.try_recv() {
                Ok(new_config) => {
                    // Only change the configuration if it's really new
                    if new_config != old_config {
                        let caps = create_caps(new_config);
                        appsrc.set_caps(&caps);
                        height = new_config.height;
                        width = new_config.width;
                        target_size = cv::Size2i::new(width as i32, height as i32);
                    }
                }
                Err(_) => {
                    trace!("nothing on the channel");
                }
            }
            if let Some(mut buffer) = buffer_pool.acquire_buffer() {
                match sched_rx.recv() {
                    Ok(frame) => {
                        let frame = frame.resize_to(target_size, InterpolationFlag::InterLinear);
                        buffer.map_write(|mapping| {
                                unsafe { copy(frame.data(), mapping.data, height * width * 3) };
                            })
                            .unwrap();
                        appsrc.push_buffer(buffer);
                        debug!("appsrc: new sample with size {}x{}", frame.cols, frame.rows);
                    }
                    Err(_) => {
                        debug!("Appsrc: error in receiving frame");
                        appsrc.end_of_stream();
                        break;
                    }
                }
            } else {
                debug!("Appsrc: error in getting buffer");
                appsrc.end_of_stream();
                break;
            }
        }
    });

    // Appsink handling
    thread::spawn(move || {
        let mut sink_count = 0;
        loop {
            match appsink.recv() {
                Ok(gst::appsink::Message::NewPreroll(_sample)) => {
                    trace!("Appsink: preroll");
                }
                Ok(gst::appsink::Message::NewSample(sample)) => {
                    let buffer = sample.buffer().expect("extracting buffer");
                    let size = buffer.size() as usize;
                    let mut vec = Vec::<u8>::with_capacity(size);
                    buffer.map_read(|mapping| {
                            debug!("appsink new sample with size: {}", size);
                            unsafe {
                                vec.set_len(size);
                                copy(mapping.data, vec.as_mut_ptr(), size);
                            }
                        })
                        .expect("failed to read data");
                    match out_tx.send(vec) {
                        Ok(_) => {
                            sink_count += 1;
                            trace!("Appsink: send appsink message ({}) to other thread",
                                   sink_count);
                        }
                        Err(_) => {
                            debug!("Appsink: Other thread has been closed, quitting");
                            break;
                        }
                    }
                }
                Ok(gst::appsink::Message::Eos) => {
                    debug!("Appsink: end of stream");
                }
                Err(_) => {
                    debug!("Appsink: thread channel closed, quitting");
                    break;
                }
            }
        }
    });

    Ok((out_rx, loader_tx))
}
