use std::sync::mpsc::Receiver;
use errors::*;
use gst::{self, MainLoop, AppSrc, AppSink, Message, BufferPool, Caps, Pipeline};

use super::skip_to_fps;
use super::loader::VideoConfig;

pub struct GstHandle {
    appsrc: AppSrc,
    appsink: AppSink,
    buffer_pool: BufferPool,
}

impl GstHandle {
    pub fn to_tuple(self) -> (AppSrc, AppSink, BufferPool) {
        (self.appsrc, self.appsink, self.buffer_pool)
    }
}

pub fn gst_main_loop(config: VideoConfig) -> Result<GstHandle> {
    gst::init();
    let mut mainloop = MainLoop::new();
    mainloop.spawn();

    let (handle, bus_recv) = create_pipeline(config)?;

    ::std::thread::spawn(move || {
        // Here runs the main loop
        for message in bus_recv.iter() {
            match message.parse() {
                gst::Message::StateChangedParsed { ref old, ref new, .. } => {
                    debug!("Main: element `{}` changed from {:?} to {:?}",
                           message.src_name(),
                           old,
                           new);
                }
                gst::Message::ErrorParsed { ref error, ref debug, .. } => {
                    debug!("Main: error msg from element `{}`: {}, {}. Quitting",
                           message.src_name(),
                           error.message(),
                           debug);
                    break;
                }
                gst::Message::Eos(_) => {
                    debug!("Main: eos received quiting");
                    break;
                }
                _ => {
                    debug!("Main: msg of type `{}` from element `{}`",
                           message.type_name(),
                           message.src_name());
                }
            }
        }

        mainloop.quit();
    });
    Ok(handle)
}

fn fps_to_string(fps: f64) -> String {
    let fps = (fps * 10.0).round() / 10.0;
    let str = {
        if fps == 30.0 {
            "30/1"
        } else if fps == 10.0 {
            "10/1"
        } else if fps == 5.0 {
            "5/1"
        } else if fps == 3.3 {
            "10/3"
        } else if fps == 2.5 {
            "5/2"
        } else if fps == 3.0 {
            "3/1"
        } else if fps == 2.0 {
            "2/1"
        } else if fps == 1.0 {
            "1/1"
        } else {
            panic!("unsupported fps {}", fps);
        }
    };
    String::from(str)
}

pub fn create_caps(config: VideoConfig) -> Caps {
    let fps = skip_to_fps(config.skip);
    let caps = format!("video/x-raw,format=BGR,width={},height={},framerate={}",
                       config.width,
                       config.height,
                       fps_to_string(fps));
    trace!("Created pipeline with caps: {}", caps);
    Caps::from_string(&caps).expect("failed to create caps from string")
}

pub fn create_pipeline(config: VideoConfig) -> Result<(GstHandle, Receiver<Message>)> {
    let caps = create_caps(config);
    let quantizer = config.quantizer;
    let pipeline_str = format!("appsrc name=appsrc0 ! videoconvert ! x264enc tune=zerolatency \
                                pass=5 speed-preset=1 quantizer={} threads=4 bitrate=2048000 ! \
                                appsink name=appsink0",
                               quantizer);

    // Create the pipeline
    let mut pipeline = Pipeline::new_from_str(&pipeline_str)?;
    let mut bus = pipeline.bus().expect("failed to get bus");
    let bus_recv = bus.receiver();

    // Bind appsrc
    let appsrc = pipeline.get_by_name("appsrc0").expect("failed to find appsrc");
    let mut appsrc = AppSrc::new_from_element(appsrc);
    appsrc.set_caps(&caps);

    let appsink = pipeline.get_by_name("appsink0").expect("failed to find appsink");
    let appsink = AppSink::new_from_element(appsink);

    let buf_size = config.width * config.height * 3;
    let mut bufferpool = BufferPool::new().expect("failed to allocate buffer");
    bufferpool.set_params(&caps, (buf_size) as u32, 0, 0);
    assert!(bufferpool.set_active(true).is_ok());

    pipeline.play();
    let handle = GstHandle {
        appsrc: appsrc,
        appsink: appsink,
        buffer_pool: bufferpool,
    };
    Ok((handle, bus_recv))
}
