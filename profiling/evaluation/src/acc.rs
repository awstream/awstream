use super::VideoConfig;
use csv;
use itertools::Itertools;
use std::collections::HashMap;
use std::io::Read;

/// Detection represents detected object. This struct is mostly constructed from
/// the CSV log.
///
/// ```ignore
/// 000001, 124.38, tvmonitor, 0.44172388, 0.13305521, 0.24362181, 0.13837068, 0.22568882
/// 000001, 124.38, laptop, 0.26606864, 0.3507892, 0.25254864, 0.091369726, 0.18242516
/// 000001, 124.38, bottle, 0.46499825, 0.21541335, 0.59823406, 0.032152083, 0.10516674
/// 000001, 124.38, cup, 0.3431328, 0.18984382, 0.6296689, 0.034560524, 0.089387275
/// 000001, 124.38, person, 0.25827762, 0.32786608, 0.7164561, 0.2547089, 0.47715223
/// 000001, 124.38, chair, 0.40833116, 0.061372586, 0.7874061, 0.115715936, 0.43029734
/// 000001, 124.38, chair, 0.816612, 0.59668547, 0.82424617, 0.33602527, 0.37017298
/// ```
#[derive(RustcDecodable, Clone, Debug)]
pub struct Detection {
    frame_num: usize,
    time: f64,
    label: String,
    _prob: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// A FrameDetections contains the frame number and all the objects detected in
/// this frame (hence a vector of Detection).
#[derive(Debug)]
pub struct FrameDetections {
    pub frame_num: usize,
    dets: Vec<Detection>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Stat {
    true_positive: usize,
    false_positive: usize,
    false_negative: usize,
}

impl Stat {
    fn new(tp: usize, fp: usize, fneg: usize) -> Self {
        Stat {
            true_positive: tp,
            false_positive: fp,
            false_negative: fneg,
        }
    }
}

pub struct FrameStat {
    pub frame_num: usize,
    pub config: VideoConfig,
    pub stat: Stat,
}

impl FrameStat {
    pub fn new(i: usize, config: VideoConfig, stat: Stat) -> Self {
        FrameStat {
            frame_num: i,
            config: config,
            stat: stat,
        }
    }

    /// Convert a frame stat to tuple
    pub fn to_tuple(&self) -> (usize, usize, usize, usize, usize, usize, usize) {
        (
            self.frame_num,
            self.config.width,
            self.config.skip,
            self.config.quant,
            self.stat.true_positive,
            self.stat.false_positive,
            self.stat.false_negative,
        )
    }
}

impl FrameDetections {
    /// Count the number of true positive detections in this frame. True
    /// positive is defined with the `valid_against` function.
    ///
    /// If every object is detected, return the number of objects in the
    /// groundtruth (maximally allowed correct detection).
    fn true_positive(&self, groundtruth: &FrameDetections) -> usize {
        let count = self.dets
            .iter()
            .filter(|d| groundtruth.dets.iter().any(|gt| d.valid_against(gt)))
            .count();
        ::std::cmp::min(count, groundtruth.dets.len())
    }

    /// Returns Stat (true positive, false positive, false negative)
    pub fn stat_against(&self, groundtruth: &FrameDetections) -> Stat {
        let tp_and_fp = self.dets.len();
        let tp_and_fn = groundtruth.dets.len();
        let tp = self.true_positive(groundtruth);
        Stat::new(tp, tp_and_fp - tp, tp_and_fn - tp)
    }
}

pub enum LoadAccOption {
    All,
    Until(usize),
}

/// Take a reader (file, string, etc.) and return a vector of framed detections.
pub fn load_accuracy<R: Read>(rdr: R, opt: LoadAccOption) -> Vec<FrameDetections> {
    // first create a csv reader
    let mut reader = csv::Reader::from_reader(rdr).has_headers(false);

    // decode all rows
    let data = reader
        .decode()
        .map(|record| record.expect("unexpected data format"))
        .collect::<Vec<Detection>>();

    let last_frame_num = {
        match opt {
            LoadAccOption::All => data.last().expect("at least one line").frame_num,
            LoadAccOption::Until(n) => n,
        }
    };

    // group by the frame number and for each group create a `FrameDetections`.
    // Because there are missing frames (nothing from the log), this result is
    // partial. We do another processing afterwards.
    let mut partial = data.iter()
        .group_by(|d| d.frame_num)
        .into_iter()
        .map(|(frame_num, group)| {
            let fd = FrameDetections {
                frame_num: frame_num,
                dets: group.map(|i| i.clone()).collect::<Vec<Detection>>(),
            };
            (frame_num, fd)
        })
        .collect::<HashMap<usize, FrameDetections>>();

    // for all requested data (0..last_frame_num), we find it from the partial
    // results. If there is no such data (meaning this frame has no detection,
    // return a `FrameDetections` with dets being empty vector
    (0..last_frame_num)
        .map(|i| {
            let frame_num = i + 1;
            partial.remove(&frame_num).unwrap_or(FrameDetections {
                frame_num: i,
                dets: Vec::new(),
            })
        })
        .collect::<Vec<FrameDetections>>()
}

#[inline]
fn load_groundtruth(dir: &str, option: LoadAccOption) -> Vec<FrameDetections> {
    let gt_file = super::gt_file(dir);
    load_accuracy(gt_file, option)
}

#[inline]
fn load_test(dir: &str, vc: VideoConfig, frame_num: usize) -> Vec<FrameDetections> {
    let acc_file = vc.open_acc_file(dir);
    load_accuracy(acc_file, LoadAccOption::Until(frame_num))
}

/// For a particular configuration, this function will return all the stats (for
/// all frames) against the groundtruth.
fn get_vec_of_stats(dir: &str, vc: VideoConfig, l: Option<usize>) -> Vec<Stat> {
    let option = match l {
        Some(l) => LoadAccOption::Until(l),
        None => LoadAccOption::All,
    };

    let groundtruth = load_groundtruth(dir, option);
    let test = load_test(dir, vc, groundtruth.len());

    groundtruth
        .iter()
        .enumerate()
        .map(|(frame_num, gt_frame)| {
            let test_frame_num = frame_num.wrapping_div(vc.skip + 1);

            let ref test_frame = {
                if test_frame_num < test.len() {
                    &test[test_frame_num]
                } else {
                    &test[test.len() - 1]
                }
            };
            let stat = test_frame.stat_against(gt_frame);
            trace!("{} {} {:?}", frame_num, test_frame_num, stat);
            stat
        })
        .collect::<Vec<Stat>>()
}

/// Generate per-frame stat with configuration.
pub fn get_frame_stats(dir: &str, vc: VideoConfig, limit: Option<usize>) -> Vec<FrameStat> {
    let stats = get_vec_of_stats(dir, vc, limit);

    stats
        .iter()
        .enumerate()
        .map(|(i, stat)| FrameStat::new(i, vc, *stat))
        .collect()
}

/// This function takes an input file (accuracy measurement by frame) and
/// processes it generate an output file (accuracy by time). The granuarilty of
/// the generated file is configurable with duration (second).
pub fn aggregate_accuracy(dir: &str, outdir: &str, vc: VideoConfig, duration_in_sec: usize) {
    // Because the groundtruth is 30 frames per second, so we collect stats
    // every `duration` seconds
    let duration = duration_in_sec * 30;

    // stats is a vector of stats (tp, fp, fn) and aggregate (chunk) them with
    // duration.
    let stats = get_vec_of_stats(dir, vc, None);

    // Write out accuracy (aggregated with `duration`)
    let of = vc.derive_acc_file(outdir);
    let mut writer = csv::Writer::from_file(of).expect("failed to open outfile for acc");

    for (i, chunk) in stats.chunks(duration).enumerate() {
        let true_positive = chunk.iter().map(|i| i.true_positive).sum::<usize>();
        let false_postive = chunk.iter().map(|i| i.false_positive).sum::<usize>();
        let false_negative = chunk.iter().map(|i| i.false_negative).sum::<usize>();

        let p = precision(true_positive, false_postive);
        let r = recall(true_positive, false_negative);
        let f1 = f1(p, r);
        writer.encode((i, f1)).expect("failed to write csv");
    }
}

/// This function takes an input file (accuracy measurement by frame) and
/// extracts the processing time. If the frame is missing, it returns
/// `f64::NAN`.
pub fn extract_proc_time(dir: &str, outdir: &str, vc: VideoConfig) {
    // Input
    let acc_file = vc.open_acc_file(dir);
    let test = load_accuracy(acc_file, LoadAccOption::All);

    // Output
    let outfile = vc.derive_ts_file(outdir);
    let mut writer = csv::Writer::from_file(outfile).expect("failed to open outfile for time");

    for (i, frame_det) in test.iter().enumerate() {
        let record = {
            if frame_det.dets.len() > 0 {
                (frame_det.frame_num, frame_det.dets.first().unwrap().time)
            } else {
                (i, ::std::f64::NAN)
            }
        };
        writer.encode(record).expect("failed to write csv");
    }
}

pub fn precision(tp: usize, fp: usize) -> f64 {
    1.0 * (tp as f64) / ((tp + fp) as f64)
}

pub fn recall(tp: usize, fnn: usize) -> f64 {
    1.0 * (tp as f64) / ((tp + fnn) as f64)
}

pub fn f1(precision: f64, recall: f64) -> f64 {
    2.0 * precision * recall / (precision + recall)
}

impl Detection {
    pub fn to_rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    // If IOU is larger than 0.5
    pub fn valid_against(&self, gt: &Detection) -> bool {
        // check label the same
        let iou = self.to_rect().iou_with(gt.to_rect());
        self.label == gt.label && iou > 0.5
    }
}

#[derive(Debug)]
pub struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x: x,
            y: y,
            w: w,
            h: h,
        }
    }

    pub fn area(&self) -> f64 {
        self.w * self.h
    }

    pub fn iou_with(&self, other: Rect) -> f64 {
        // first get the intersection area
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let w = (self.x + self.w).min(other.x + other.w) - x;
        let h = (self.y + self.h).min(other.y + other.h) - y;

        if w < 0.0 || h < 0.0 {
            0.0
        } else {
            let intersection = w * h;
            intersection / (self.area() + other.area() - intersection)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_rect_iou() {
        let rect1 = Rect::new(1.0, 1.0, 2.0, 2.0);
        let rect2 = Rect::new(1.0, 1.0, 2.0, 2.0);
        assert_eq!(rect1.iou_with(rect2), 1.0);

        let rect1 = Rect::new(1.0, 1.0, 2.0, 2.0);
        let rect2 = Rect::new(3.0, 3.0, 2.0, 2.0);
        assert_eq!(rect1.iou_with(rect2), 0.0);

        let rect1 = Rect::new(1.0, 1.0, 2.0, 2.0);
        let rect2 = Rect::new(2.0, 2.0, 2.0, 2.0);
        assert_eq!(rect1.iou_with(rect2), 1.0 / (4.0 + 4.0 - 1.0));
    }

    #[test]
    fn test_frame_detection_true_positive() {
        let gt_str = "
000001, 1.0, obj1, 0.5, 0.1, 0.1, 0.2, 0.2
000001, 1.0, obj2, 0.5, 0.4, 0.4, 0.2, 0.2
000002, 1.0, obj1, 0.5, 0.1, 0.1, 0.2, 0.2";
        let gt = load_accuracy(gt_str.as_bytes(), None);

        let test_str = "
000001, 1.0, obj1, 0.5, 0.1, 0.1, 0.2, 0.2";
        let test = load_accuracy(test_str.as_bytes(), None);

        // Groundtruth has two frames and test has only one frame
        assert_eq!(gt.len(), 2);
        assert_eq!(test.len(), 1);

        // Test's first frame should has true positive: 1, false positive: 0,
        // false negative: 0
        let stat = test[0].stat_against(&gt[0]);
        assert_eq!(stat.0, 1);
        assert_eq!(stat.1, 0);
        assert_eq!(stat.2, 1);
    }

    #[test]
    fn test_empty_file() {
        let gt_str = "
000001, 1.0, obj1, 0.5, 0.1, 0.1, 0.2, 0.2
000001, 1.0, obj2, 0.5, 0.4, 0.4, 0.2, 0.2
000002, 1.0, obj1, 0.5, 0.1, 0.1, 0.2, 0.2";
        let gt = load_accuracy(gt_str.as_bytes(), None);

        let test_str = "";
        let test = load_accuracy(test_str.as_bytes(), Some(gt.len()));

        // Groundtruth has two frames and test has only one frame
        assert_eq!(gt.len(), 2);
        assert_eq!(test.len(), 2);
    }
}
