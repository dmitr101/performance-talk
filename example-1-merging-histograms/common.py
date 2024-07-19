from dataclasses import dataclass, asdict


@dataclass
class Bucket:
    min: float
    max: float
    count: int


@dataclass
class Histogram:
    buckets: list[Bucket]
