# minirun

[![ci](https://github.com/ip888/minirun/actions/workflows/ci.yml/badge.svg)](https://github.com/ip888/minirun/actions/workflows/ci.yml)

Measure cold-start time of any containerised workload — p50 / p99 / mean
over N iterations.

> **Status:** all planned phases shipped. See [Roadmap](#roadmap) below.

Measured cold-start (5 runs each, 2026-11, Intel Core i5-5250U laptop,
Docker 29.7): **nginx 1447 ms**, **Postgres 17 3424 ms**, **Spring Boot
(Petclinic) 14065 ms** — p50.

## What it does

Spawns a Docker container, polls an HTTP (HyperText Transfer Protocol)
health endpoint until it returns 200 OK — or, for services without HTTP,
a TCP (Transmission Control Protocol) port — measures wall-clock from
`docker run` to the first healthy response, repeats N times, and reports
p50 / p99 / mean / stddev. Language-agnostic — works with nginx,
Spring Boot, Node.js, Postgres, Redis, anything with an HTTP endpoint or
a TCP port.

## Install

Requires Docker on the machine that runs `minirun`.

```sh
cargo install --git https://github.com/ip888/minirun
minirun --help
```

## Examples

HTTP service (real run, nginx):

```
$ minirun --image nginx:latest \
          --wait-url http://localhost:8080/ \
          --publish 8080:80 \
          --iters 5

cold start (5 runs):
 p50: 1447 ms
 p99: 1859 ms
 mean ± stddev: 1488 ± 206 ms
 min: 1238 ms
 max: 1859 ms
```

Progress lines (`Iteration 1/5`, per-run timings, errors) go to stderr;
only the results go to stdout.

JVM (Java Virtual Machine) service (real run, Spring Boot Petclinic with
Actuator health endpoint):

```
$ minirun --image springio/petclinic \
          --publish 8080:8080 \
          --wait-url http://localhost:8080/actuator/health \
          --timeout 120 \
          --iters 5

cold start (5 runs):
 p50: 14065 ms
 p99: 16121 ms
 mean ± stddev: 14336 ± 1264 ms
 min: 12402 ms
 max: 16121 ms
```

Services without HTTP (real run, Postgres):

```
$ minirun --image postgres:17 \
          -e POSTGRES_PASSWORD=x \
          --publish 5432:5432 \
          --wait-tcp localhost:5432 \
          --iters 5

cold start (5 runs):
 p50: 3424 ms
 p99: 4684 ms
 mean ± stddev: 3641 ± 525 ms
 min: 3308 ms
 max: 4684 ms
```

## Options

| Flag          | Default                 | Meaning                                                         |
| ------------- | ----------------------- | --------------------------------------------------------------- |
| `--image`     | (required)              | Docker image to run                                             |
| `--publish`   | `8080:80`               | Port forwarding, `host:container`                               |
| `-e`, `--env` | (none)                  | `KEY=VALUE` passed to the container, repeatable                 |
| `--wait-url`  | `http://localhost:8080` | HTTP URL polled until it answers 200 OK                         |
| `--wait-tcp`  | (none)                  | `host:port` probed instead of HTTP; conflicts with `--wait-url` |
| `--iters`     | `5`                     | Number of cold-start iterations                                 |
| `--timeout`   | `60`                    | Seconds to wait for readiness, per iteration                    |
| `--output`    | `human`                 | `human` (table) or `json` (for CI pipelines)                    |

Failed iterations (timeout, Docker error) are reported and skipped. If
any iteration fails, `minirun` exits with code 1.

### How `--wait-tcp` decides "ready"

With `--publish`, Docker's `docker-proxy` accepts TCP connections on the
host port immediately — before the service inside has started. So a
successful connect alone means nothing. `minirun` connects, then reads
for 200 ms:

- connection closed (EOF) or reset → only the proxy answered, retry;
- connection stays open and silent → real server waiting for the client
  (Postgres, Redis) → ready;
- data arrives → real server that speaks first (MySQL, SSH) → ready.

The measurement uses the moment of that connect, so the 200 ms window
is not added to the result.

### JSON output

`--output json` prints a single JSON (JavaScript Object Notation) object
to stdout, so it can be piped straight into `jq`:

```
$ minirun --image nginx:latest --iters 3 --output json 2>/dev/null | jq .
{
  "runs": 3,
  "p50_ms": 1283,
  "p99_ms": 1306,
  "mean_ms": 1255,
  "stddev_ms": 56,
  "min_ms": 1175,
  "max_ms": 1306
}
```

## Why this project

A personal Rust practice project. The goal is to measure the full
cold-start of a container end to end from a simple CLI (Command Line
Interface) tool, learning along the way:

- `clap` derive for CLI argument parsing
- `tokio` async runtime
- `reqwest` for HTTP polling
- `std::process::Command` for spawning Docker
- `std::time::Instant` for monotonic timing
- `thiserror` for structured error enums
- `serde` + `serde_json` for machine-readable output
- GitHub Actions CI (Continuous Integration)

Not a Firecracker / rust-nano-vm replacement — those run their own VMs.
`minirun` is a thin benchmark harness around any container runtime.

## Build

Requires Rust 1.88+ (edition 2024, `if let` chains).

```sh
cargo build --release
./target/release/minirun --help
```

## Roadmap

| Phase | Status  | What                                                                 |
| ----- | ------- | -------------------------------------------------------------------- |
| 0     | shipped | Scaffold, `main` prints version                                      |
| 1     | shipped | CLI (`clap` derive): `--image`, `--iters`, `--wait-url`, `--publish` |
| 2     | shipped | Spawn Docker container via `std::process::Command`                   |
| 3     | shipped | Monotonic timing with `Instant`                                      |
| 4     | shipped | Async HTTP health polling (`tokio` + `reqwest`)                      |
| 5     | shipped | Loop N iterations, collect timings                                   |
| 6     | shipped | Statistics (p50 / p99 / mean / stddev)                               |
| 7     | shipped | JSON (JavaScript Object Notation) output mode                        |
| 8     | shipped | Error enum via `thiserror`                                           |
| 9     | shipped | Unit + integration tests                                             |
| 9.5   | shipped | Bonus: `--wait-tcp` for non-HTTP services (Postgres), `-e KEY=VALUE` |
| 10    | shipped | CI badge + README polish                                             |

## License

MIT. See [LICENSE](LICENSE).

## Author

[Ihor Petrov](https://github.com/ip888)
