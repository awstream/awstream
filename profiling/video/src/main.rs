extern crate env_logger;
extern crate video_analytics;
extern crate cv;
extern crate time;
extern crate darknet;
use cv::cuda::GpuHog as Hog;
use cv::objdetect::{HogParams, ObjectDetect, SvmDetector};
use darknet::*;
use std::env;
use std::fs::File;
use std::io::Write;

use video_analytics::loader::*;

fn main() {
    env_logger::init().unwrap();

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() > 1 {
        if args[1] == "darknet" {
            darknet();
        } else if args[1] == "pedestrian" {
            pedestrian();
        }
        ::std::process::exit(0);
    }

    let skip = env::var("SKIP")
        .unwrap_or("0".to_string())
        .parse::<usize>()
        .expect("invalid SKIP via environment variable");

    let width = env::var("WIDTH")
        .unwrap_or("1920".to_string())
        .parse::<usize>()
        .expect("invalid WIDTH via environment variable");

    let quantizer = env::var("Q")
        .unwrap_or("20".to_string())
        .parse::<usize>()
        .expect("invalid Q via environment variable");

    let fname = env::var("FILE")
        .unwrap_or("output".to_string())
        .parse::<String>()
        .expect("invalid FILE via environment variable");

    let height = width / 16 * 9;

    let path = env::var("INPUT").expect("please specify the path for input images");
    let ext = env::var("EXT").expect("please specify the extension for input images");

    let lc = LoaderConfig {
        path: path,
        ext: ext,
        circular: false,
    };

    let config = VideoConfig {
        width: width,
        height: height,
        skip: skip,
        quantizer: quantizer,
    };
    let (loader, _loader_ctl) = load_x264(lc, config).unwrap();

    let mut i = 1;
    let mut sink_file = File::create(&format!("{}", fname)).unwrap();
    loop {
        // println!("{} ms", elapsed.subsec_nanos() / 1_000_000);
        let encoded = loader.recv().expect("failed to receive encoded");
        sink_file.write(&encoded).expect("failed to write to file sink");
        println!("{}, {}", i, encoded.len());
        i += 1;
    }
}

fn cv_mat_to_darknet_image(mat: &cv::Mat) -> darknet::InputImage {
    let data: *const u8 = mat.data();
    let h = mat.rows;
    let w = mat.cols;
    let c = mat.channels;

    let mut out = darknet::InputImage::new(w, h, c);
    let out_data = out.data_mut();
    let mut count = 0;
    for k in 0..c {
        for y in 0..h {
            for x in 0..w {
                let offset = (c * (w * y + x) + k) as isize;
                unsafe {
                    let v = *(data.offset(offset)) as f32 / 255.0;
                    *out_data.offset(count) = v;
                }
                count += 1;
            }
        }
    }
    out
}

fn pedestrian() {
    let path = env::var("INPUT").expect("please specify the path for input video");
    // let cap = cv::videoio::VideoCapture::from_path(&path);

    // Prepare HOG detector
    let mut params = HogParams::default();
    params.hit_threshold = 0.3;
    let mut hog = Hog::with_params(params);
    let detector = SvmDetector::default_people_detector();
    hog.set_svm_detector(detector);

    let mut frame_no = 1;
    for i in 1..8000 {
        // while let Some(image) = cap.read() {
        // let image = image.cvt_color(cv::imgproc::ColorConversionCodes::BGR2RGB);
        let f = format!("{}/{:06}.bmp", path, i);
        println!("{}", f);
        let image = cv::Mat::from_path(&f, cv::imgcodecs::ImreadModes::ImreadGrayscale).unwrap();
        //    while let Some(image) = cap.read() {
        //        let image = image.cvt_color(cv::imgproc::ColorConversionCodes::BGR2GRAY);
        let time = ::std::time::Instant::now();
        // Result is a vector of tuple (Rect, conf: f64). See documentation
        // of hog detection if you are confused.
        let result = hog.detect(&image);
        let elapsed = time.elapsed();
        let proc_time = elapsed.as_secs() as f64 * 1_000.0 +
                        elapsed.subsec_nanos() as f64 / 1_000_000.0;

        for r in &result {
            let normalized = r.0.normalize_to_mat(&image);
            println!("{:06}, {:.02}, {}, {}, {}, {}, {}, {}",
                     frame_no,
                     proc_time,
                     "pedestrian",
                     r.1,
                     normalized.x,
                     normalized.y,
                     normalized.width,
                     normalized.height);
        }

        frame_no += 1;
    }
}

fn darknet() {
    let path = env::var("INPUT").expect("please specify the path for input video");
    // let cap = cv::videoio::VideoCapture::from_path(&path);

    let mut dn = Darknet::new(concat!(env!("CARGO_MANIFEST_DIR"), "/darknet-data/coco.data"),
                              concat!(env!("CARGO_MANIFEST_DIR"), "/darknet-data/yolo.cfg"),
                              concat!(env!("CARGO_MANIFEST_DIR"), "/darknet-data/yolo.weights"),
                              concat!(env!("CARGO_MANIFEST_DIR"), "/darknet-data/coco.names"));

    let mut frame_no = 1;
    for index in 1..20000 {
        // while let Some(image) = cap.read() {
        let f = format!("{}/{:06}.bmp", path, index);
        let image = cv::Mat::from_path(&f, cv::imgcodecs::ImreadModes::ImreadColor).unwrap();
        let image = image.cvt_color(cv::imgproc::ColorConversionCodes::BGR2RGB);
        let image = cv_mat_to_darknet_image(&image);
        let detections = dn.detect(image);
        for i in 0..detections.num {
            let ref d = detections.detections[i];
            println!("{:06}, {:.02}, {}",
                     frame_no,
                     detections.proc_time_in_ms,
                     d.csv());
        }
        frame_no += 1;
    }
}
