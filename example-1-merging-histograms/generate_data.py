import random
import argparse
import json
from dataclasses import dataclass, asdict

from common import Bucket, Histogram


def generate_random_histograms(
    min: float, max: float, num_histograms: int, num_buckets: int
) -> list[Histogram]:
    histograms = []
    for _ in range(num_histograms):
        buckets = []
        for b in range(num_buckets):
            min_bucket = min + (max - min) * b / num_buckets
            max_bucket = min + (max - min) * (b + 1) / num_buckets
            count = random.randint(0, 100)
            buckets.append(Bucket(min_bucket, max_bucket, count))
        random.shuffle(buckets)
        histograms.append(Histogram(buckets))
    return histograms


def save_histograms_to_json(histograms, filename):
    with open(filename, "w") as f:
        json.dump([asdict(histogram) for histogram in histograms], f, indent=4)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Generate random histograms and save them as a JSON file."
    )
    parser.add_argument(
        "min_val", type=float, help="Minimum value for the histogram buckets"
    )
    parser.add_argument(
        "max_val", type=float, help="Maximum value for the histogram buckets"
    )
    parser.add_argument(
        "num_histograms", type=int, help="Number of histograms to generate"
    )
    parser.add_argument(
        "num_buckets", type=int, help="Number of buckets in each histogram"
    )
    parser.add_argument(
        "output_file", type=str, help="Output file name for the JSON data"
    )
    args = parser.parse_args()

    min_val = args.min_val
    max_val = args.max_val
    num_histograms = args.num_histograms
    num_buckets = args.num_buckets
    output_file = args.output_file

    histograms = generate_random_histograms(
        min_val, max_val, num_histograms, num_buckets
    )
    save_histograms_to_json(histograms, output_file)
