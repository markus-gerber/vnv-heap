use std::io::stdout;

use core::hint::black_box;
use serde::Serialize;

mod allocate_max;
mod allocate_min;

pub use allocate_max::*;
pub use allocate_min::*;

pub trait Benchmark<O: Serialize> {
    fn get_name(&self) -> &'static str;

    fn get_bench_options(&self) -> &O;

    fn prepare_next_iteration(&mut self);

    fn execute<T: Timer>(&mut self) -> u32;

    fn run_benchmark<T: Timer>(mut self, options: BenchmarkRunOptions) -> BenchmarkRunResult
    where
        Self: Sized,
    {
        assert_eq!(options.repetitions as usize, options.result_buffer.len());

        print!("# Running Benchmark \"{}\" with options ", self.get_name());
        serde_json::to_writer(stdout(), self.get_bench_options()).unwrap();
        println!();

        for _ in 0..options.cold_start {
            black_box(self.execute::<T>());
            self.prepare_next_iteration();
        }

        for i in 0..options.result_buffer.len() {
            let res = black_box(self.execute::<T>());
            options.result_buffer[i] = res;

            self.prepare_next_iteration();
        }

        let run_info = BenchmarkRunInfo {
            bench_name: self.get_name(),
            bench_options: self.get_bench_options(),
            machine_name: options.machine_name,
            cold_start: options.cold_start,
            repetitions: options.repetitions,
            data: &options.result_buffer,
        };

        print!("[BENCH-INFO] ");
        serde_json::to_writer(stdout(), &run_info).unwrap();
        println!("");

        let res = BenchmarkRunResult::from_buffer(&options.result_buffer);
        println!(
            "# Finished {}: mean={}us, min={}us, max={}us",
            self.get_name(),
            res.mean_latency,
            res.min_latency,
            res.max_latency
        );
        println!();

        res
    }
}

pub struct BenchmarkRunOptions<'a> {
    pub repetitions: u32,
    pub result_buffer: &'a mut [u32],

    pub cold_start: u32,

    pub machine_name: &'static str,
}

#[derive(Serialize)]
pub struct BenchmarkRunInfo<'a, O: Serialize> {
    bench_name: &'static str,
    bench_options: &'a O,
    machine_name: &'static str,
    cold_start: u32,
    repetitions: u32,
    data: &'a [u32],
}

pub struct BenchmarkRunResult {
    pub mean_latency: u32,
    pub min_latency: u32,
    pub max_latency: u32,
}

impl BenchmarkRunResult {
    fn from_buffer(buffer: &[u32]) -> Self {
        Self {
            mean_latency: buffer.iter().map(|x| *x).sum::<u32>() / (buffer.len() as u32),
            min_latency: buffer.iter().min().map(|x| *x).unwrap(),
            max_latency: buffer.iter().max().map(|x| *x).unwrap(),
        }
    }
}

pub trait Timer {
    fn start() -> Self;

    fn stop(self) -> u32;
}
