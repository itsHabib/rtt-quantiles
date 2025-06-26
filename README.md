# RTT Quantile Project

This project consists of two main applications:

1. **rtt-quantiles**: An eBPF-based collector that hooks into the Linux kernel's `tcp_rcv_established` function to capture TCP RTT data and stores t-digests in DynamoDB.

2. **rtt-api**: A REST API service that retrieves t-digests from DynamoDB for a specified time range, merges them, and returns quantile calculations.

## rtt-quantiles

This application uses eBPF to hook into the Linux kernel's TCP stack and collect RTT measurements.

### Technical Details

- Uses eBPF's `fentry` probe mechanism
- Hooks into `tcp_rcv_established` function in the kernel's TCP implementation
- Extracts `srtt_us` (smoothed round-trip time in microseconds) from the TCP socket structure
- Passes data to userspace via a ring buffer
- Processes data into t-digest structures for efficient storage of distribution data
- Stores t-digests in DynamoDB with timestamps for later analysis

### Prerequisites

1. stable rust toolchains: `rustup toolchain install stable`
2. nightly rust toolchains: `rustup toolchain install nightly --component rust-src`
3. bpf-linker: `cargo install bpf-linker` (`--no-default-features` on macOS)

### Build & Run

Build and run the eBPF collector:

```shell
cargo build --package rtt-quantiles --release
sudo -E cargo run --package rtt-quantiles --release
```

## rtt-api

This application provides a REST API to query RTT data and calculate quantiles from the stored t-digests.

### Technical Details

- Queries t-digests from DynamoDB based on a specified time range
- Merges multiple t-digests to maintain statistical accuracy
- Calculates quantiles (p50, p75, p90, p95, p99) from the merged t-digest
- Returns results via a JSON REST API

### Build & Run

Build and run the API server:

```shell
cargo build --package rtt-api --release
cargo run --package rtt-api --release
```

The API will be available at http://localhost:8080

### API Usage

Query RTT quantiles with:

```shell
curl "http://localhost:8080/quantiles?from=2023-06-01T00:00:00Z&to=2023-06-30T23:59:59Z"
```

Response format:
```json
{
  "agg_level": "1m",
  "sample_count": 2715326,
  "quantiles": {
    "p50": "4.013",
    "p75": "10.638",
    "p90": "16.873",
    "p95": "18.786",
    "p99": "21.500"
  }
}
```

## Cross-compiling on macOS

Cross compilation for the rtt-quantiles application:

```shell
CC=${ARCH}-linux-musl-gcc cargo build --package rtt-quantiles --release \
  --target=${ARCH}-unknown-linux-musl \
  --config=target.${ARCH}-unknown-linux-musl.linker=\"${ARCH}-linux-musl-gcc\"
```

*Note: used for learning purposes, not production use*