# minirun

Measure cold-start time of any containerised workload — p50 / p99 / mean
over N iterations.

> **Status:** work in progress. Scaffold stage (Phase 0 of 10).
> See [Roadmap](#roadmap) below.

## What it does

Spawns a Docker container, polls an HTTP (HyperText Transfer Protocol)
health endpoint until it returns 200 OK, measures wall-clock from
`docker run` to the first healthy response, repeats N times, and reports
p50 / p99 / mean / stddev. Language-agnostic — works with nginx,
Postgres, Spring Boot, Node.js, anything that speaks HTTP.

Planned output:

```
$ minirun --image nginx:latest \
          --wait-url http://localhost:8080/ \
          --publish 8080:80 \
          --iters 10

cold start (10 runs):
  p50:   124 ms
  p99:   188 ms
  mean:  132 ms ± 21 ms
  min:   103 ms
  max:   201 ms
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

Requires Rust stable (1.80+ probably works, 1.90+ definitely).

```sh
cargo build --release
./target/release/minirun --help
```

## Roadmap

| Phase | Status  | What                                                                   |
|-------|---------|------------------------------------------------------------------------|
| 0     | shipped | Scaffold, `main` prints version                                        |
| 1     | TODO    | CLI (`clap` derive): `--image`, `--iters`, `--wait-url`, `--publish`   |
| 2     | TODO    | Spawn Docker container via `std::process::Command`                     |
| 3     | TODO    | Monotonic timing with `Instant`                                        |
| 4     | TODO    | Async HTTP health polling (`tokio` + `reqwest`)                        |
| 5     | TODO    | Loop N iterations, collect timings                                     |
| 6     | TODO    | Statistics (p50 / p99 / mean / stddev)                                 |
| 7     | TODO    | JSON (JavaScript Object Notation) output mode                          |
| 8     | TODO    | Error enum via `thiserror`                                             |
| 9     | TODO    | Unit + integration tests                                               |
| 10    | TODO    | CI badge + README polish                                               |

## License

MIT. See [LICENSE](LICENSE).

## Author

[Ihor Petroff](https://github.com/ip888)
