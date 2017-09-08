use super::VideoConfig;
use csv;
use helper;

/// This function takes an input file (bandwidth measurement by frame) and
/// processes it generate an output file (bandwidth by time). The granuarilty of
/// the generated file has a configurable resolution (`duration_in_sec`).
pub fn aggregate_bandwidth(dir: &str, outdir: &str, vc: VideoConfig, duration: usize) {
    // format input file name
    let infile = vc.derive_bw_file(dir);
    let outfile = vc.derive_bw_file(outdir);

    // calculate how many frames we need to group
    let fps = helper::skip_to_fps(vc.skip);

    // reader and writer for the input/output file
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(&infile)
        .expect("failed to open bandwidth file");
    let mut writer = csv::Writer::from_path(outfile).expect("failed to open outfile");

    // read input data as a vector
    // it must follow `frame_num, size` format
    let data = reader
        .deserialize()
        .map(|record| record.expect("unexpected data format"))
        .collect::<Vec<(usize, usize)>>();

    // iterate over windows and write the bandwidth (in mbps). it must follow
    // `frame_num, size` format.
    for (i, chunk) in data.chunks(fps * duration).enumerate() {
        let bw = (chunk.iter().map(|i| i.1).sum::<usize>() * 8) as f64 / 1_000_000.0 /
            (duration as f64);
        writer.serialize((i, bw)).expect("failed to write bw to csv");
    }
}
