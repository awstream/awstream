use super::VideoConfig;

/// Converts skip per second to frames per second
pub fn skip_to_fps(skip: usize) -> usize {
    ((30.0 / (skip as f64 + 1.0) * 10.0).round() / 10.0) as usize
}

/// Returns a list of all configurations [VideoConfig](struct.VideoConfig.html).
pub fn all_configurations() -> Vec<VideoConfig> {
    let width = vec![1920, 1600, 1280, 960, 640, 320];
    let skip = vec![0, 2, 5, 9, 14, 29];
    let quant = vec![0, 10, 20, 30, 40, 50];

    iproduct!(width, skip, quant)
        .map(|(w, s, q)| {
            VideoConfig {
                width: w,
                skip: s,
                quant: q,
            }
        })
        .collect::<Vec<_>>()
}
