use serde::{Deserialize, Serialize};
use std::fs::read_to_string;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Bucket {
    min: f32,
    max: f32,
    count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Histogram {
    buckets: Vec<Bucket>,
}

impl Histogram {
    fn new() -> Histogram {
        Histogram {
            buckets: Vec::new(),
        }
    }

    fn sort_buckets(&mut self) {
        self.buckets
            .sort_by(|a, b| a.min.partial_cmp(&b.min).unwrap());
    }

    fn merge_with(&mut self, other: &Histogram) {
        for (idx, bucket) in other.buckets.iter().enumerate() {
            self.buckets[idx].count += bucket.count;
        }
    }
}

fn merge_histograms_mut_in_place(mut histograms: Vec<Histogram>) -> Histogram {
    if histograms.is_empty() {
        return Histogram::new();
    }

    let mut result = histograms[0].clone();
    result.sort_buckets();
    for histogram in histograms.iter_mut().skip(1) {
        histogram.sort_buckets();
        result.merge_with(histogram);
    }
    result
}

fn merged_histograms_naive(histograms: Vec<Histogram>) -> Histogram {
    histograms
        .iter()
        .map(|histogram| {
            let mut sorted_histogram = histogram.clone();
            sorted_histogram.sort_buckets();
            sorted_histogram
        })
        .reduce(|mut acc, histogram| {
            acc.merge_with(&histogram);
            acc
        })
        .unwrap_or_else(Histogram::new)
}

fn calculate_percentile(histogram: &Histogram, percentile: f32) -> f32 {
    let total_count: u32 = histogram.buckets.iter().map(|bucket| bucket.count).sum();
    let target_count = total_count as f32 * percentile;
    let mut current_count = 0.0;
    for bucket in histogram.buckets.iter() {
        current_count += bucket.count as f32;
        if current_count >= target_count {
            return bucket.max;
        }
    }
    histogram.buckets.last().unwrap().max
}

fn read_data(filename: &str) -> Vec<Histogram> {
    let file_content = read_to_string(filename).expect("Failed to read file");
    serde_json::from_str(&file_content).expect("Failed to deserialize JSON")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }
    let histograms = read_data(&args[1]);

    let start_time = std::time::Instant::now();
    let merged_histogram = merge_histograms_mut_in_place(histograms);
    let elapsed_time = start_time.elapsed();

    // let start_time = std::time::Instant::now();
    // let merged_histogram = merged_histograms_naive(histograms);
    // let elapsed_time = start_time.elapsed();

    println!(
        "Merging histograms took {} seconds",
        elapsed_time.as_secs_f32()
    );
    println!(
        "Resulting p95: {}",
        calculate_percentile(&merged_histogram, 0.95)
    );
}
