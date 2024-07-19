import argparse
import json
from dataclasses import asdict
from common import Bucket, Histogram
import time


def load_histograms(file_path: str) -> list[Histogram]:
    with open(file_path) as file:
        data = json.load(file)
    return [
        Histogram(buckets=[Bucket(**bucket) for bucket in histogram["buckets"]])
        for histogram in data
    ]


def merge_histograms(histograms: list[Histogram]) -> Histogram:
    result = histograms[0]
    result.buckets.sort(key=lambda bucket: bucket.min)
    for histogram in histograms[1:]:
        for i, bucket in enumerate(
            sorted(histogram.buckets, key=lambda bucket: bucket.min)
        ):
            result.buckets[i].count += bucket.count
    return result


def calculate_percentile(histogram: Histogram, percentile: float) -> float:
    total_count = sum(bucket.count for bucket in histogram.buckets)
    target_count = total_count * percentile
    current_count = 0
    for bucket in histogram.buckets:
        current_count += bucket.count
        if current_count >= target_count:
            return bucket.max
    return histogram.buckets[-1].max


def main():
    parser = argparse.ArgumentParser(
        description="Merge multiple histograms from JSON file into a single histogram."
    )
    parser.add_argument("file", help="JSON file containing histograms to merge")
    args = parser.parse_args()

    histograms = load_histograms(args.file)

    start_time = time.time()
    result = merge_histograms(histograms)
    end_time = time.time()

    print(f"Merging histograms took {end_time - start_time:.2f} seconds.")
    print("Resulting p95: ", calculate_percentile(result, 0.95))


if __name__ == "__main__":
    main()
