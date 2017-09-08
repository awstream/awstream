Profiling and Evaluation
---

There are fives types of data involved in the profiling:

- measured data
- per-frame statistics (true positive, false positive and true negative per frame)
- summary data
- profile
- pareto

## measured data

[video-profiling](video/video-profiling) scripts will generate a folder that
contains `acc-X.csv` and `bw-X.csv`.

`acc-X.csv` is a CSV file whose entries contain the object/pedestrian detection information.

```
(frame_num, process_time, object, probability, boundingbox_x, boundingbox_y, boundingbox_width, boundingbox_height)
```

`bw-X.csv` is a CSV file whose entries contain per frame size in bytes.

```
(frame_num, size_in_bytes)
```

We manually rename `acc-1920x0x0.csv` to groundtruth file.

## statistics

[stats](evaluation/src/bin/stat.rs) takes the measured data (mainly `acc-X.csv`)
and produces statistics data. You may configure it to run for a subset of frames
`(0..limit)` or a subset of configuration with `--profile <profile_file>`.

------------------------

Note: haven't updated scripts below

## summary data

[summary.rs](summary.rs) takes the measured data (`bw-X.csv`) and the stat file
(`stats.csv`); it produces the summary data with configurable interval (5
seconds, or 1 seconds). The output is CSV files of `interval, value` where the
value could be bandwidth, accuracy or processing time.

### profile data

Takes the summary data and outputs a profile (`bw, config, accuracy`).

### pareto

Takes the profile and data and outputs a pareto (`bw, config, accuracy`).

## Summary

```
INPUT_DIR=<input directory> OUTPUT_DIR=<output directory> cargo run --bin summary
```

This will parse `$INPUT_DIR/bw-AxBxC.csv` and `$INPUT_DIR/acc-AxBxC.csv`; generate 

For bandwidth, it produces a summary of bandwidth demand for a fixed amount of
time (currently 5 seconds). If the raw video has 100 seconds worth of data, the
output will be a CSV file with 20 entries: `<second, bandwidth>` tuple.

For accuracy, similarly, it produces a summary of accuracy for a fixed amount of
time (currently 5 seconds). If the raw video has 100 seconds worth of data, the
output will be a CSV file with 20 entries: `<second, f1 score>` tuple.

While we are processing the accuracy file, the summary will also include the
processing time needed for each configuration.  This happens now for every frame
(no time-windowed aggregation). If the raw video has 100 seconds worth of data,
the output will be a CSV file with (at most) 3000 entries: `<frame_num, time>`
tuple.

## Trigger

In trigger, it evaluate different online profiling metrics. You need to supply
`$DIR` which points to the data generated from summary.

## Pareto

Pareto takes the data generated from summary and prints the Pareto-optimal set.

## Trace

Generate traces for client simulation and server to calculate accuracy on the
fly.

```
INPUT_DIR=~/box/AdaptiveStream/darknet-test-profiling-home \
OUTPUT_DIR=~/box/AdaptiveStream/tmp \
cargo run --bin trace
```


