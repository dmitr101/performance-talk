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

fn merge_histograms(mut histograms: Vec<Histogram>) -> Histogram {
    if histograms.is_empty() {
        return Histogram {
            buckets: Vec::new(),
        };
    }

    let mut result = histograms[0].clone();
    result
        .buckets
        .sort_by(|a, b| a.min.partial_cmp(&b.min).unwrap());
    for histogram in histograms.iter_mut().skip(1) {
        histogram
            .buckets
            .sort_by(|a, b| a.min.partial_cmp(&b.min).unwrap());
        for (idx, bucket) in histogram.buckets.iter_mut().enumerate() {
            result.buckets[idx].count += bucket.count;
        }
    }
    result
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }

    let file_content = read_to_string(&args[1]).expect("Failed to read file");
    let histograms: Vec<Histogram> =
        serde_json::from_str(&file_content).expect("Failed to deserialize JSON");

    let start_time = std::time::Instant::now();
    let merged_histogram = merge_histograms(histograms);
    let elapsed_time = start_time.elapsed();

    println!(
        "Merging histograms took {} seconds",
        elapsed_time.as_secs_f32()
    );
    println!(
        "Resulting p95: {}",
        calculate_percentile(&merged_histogram, 0.95)
    );
}
