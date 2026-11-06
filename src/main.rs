use clap::Parser;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// All the ways a minirun measurement can fail
#[derive(Debug, thiserror::Error)]
enum MiniRunError {
    #[error("docker: {0}")]
    Docker(String),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("timeout waiting for {target} after {secs}s")]
    Timeout { target: String, secs: u64 },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Parser)]
#[command(about = "Measure cold-start of a containerised workload")]
struct Cli {
    /// Docker image to run
    #[arg(long)]
    image: String,

    /// Port forwarding, in `host:container` form
    #[arg(long, default_value = "8080:80")]
    publish: String,

    /// Environment variable for the container, `KEY=VALUE` (repeatable)
    #[arg(short = 'e', long = "env")]
    env: Vec<String>,

    /// HTTP URL to poll for readiness
    #[arg(long, default_value = "http://localhost:8080")]
    wait_url: String,

    /// TCP `host:port` to probe for readiness instead of HTTP (Postgres, Redis, ...)
    #[arg(long, conflicts_with = "wait_url")]
    wait_tcp: Option<String>,

    /// Number of cold-start iterations to measure
    #[arg(long, default_value_t = 5)]
    iters: u32,

    /// Seconds to wait for the service to become ready, per iteration
    #[arg(long, default_value_t = 60)]
    timeout: u64,

    /// Output format for the results
    #[arg(long, value_enum, default_value = "human")]
    output: OutputFormat,
}

/// What "the service is ready" means for one measurement
enum WaitTarget {
    /// HTTP GET answers 200 OK
    Http(String),
    /// TCP connection is held by the real server, not by docker-proxy
    Tcp(String),
}

impl WaitTarget {
    fn from_cli(args: &Cli) -> Self {
        match &args.wait_tcp {
            Some(addr) => WaitTarget::Tcp(addr.clone()),
            None => WaitTarget::Http(args.wait_url.clone()),
        }
    }
}

impl std::fmt::Display for WaitTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaitTarget::Http(url) => write!(f, "http {url}"),
            WaitTarget::Tcp(addr) => write!(f, "tcp {addr}"),
        }
    }
}

/// How to print the results
#[derive(clap::ValueEnum, Clone)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug)]
struct Stats {
    p50: Duration,
    p99: Duration,
    mean: Duration,
    stddev: Duration,
    min: Duration,
    max: Duration,
}

/// Stats in JSON-friendly form: plain numbers in milliseconds
#[derive(serde::Serialize)]
struct StatsJson {
    runs: usize,
    p50_ms: u128,
    p99_ms: u128,
    mean_ms: u128,
    stddev_ms: u128,
    min_ms: u128,
    max_ms: u128,
}

impl Stats {
    fn to_json(&self, runs: usize) -> StatsJson {
        StatsJson {
            runs,
            p50_ms: self.p50.as_millis(),
            p99_ms: self.p99.as_millis(),
            mean_ms: self.mean.as_millis(),
            stddev_ms: self.stddev.as_millis(),
            min_ms: self.min.as_millis(),
            max_ms: self.max.as_millis(),
        }
    }
}

/// Panics if durations is empty
///
/// # Arguments
///
/// * `durations` - A slice of `Duration` values to compute statistics for
///
/// # Returns
///
/// A `Stats` struct containing p50, p99, mean, stddev, min, and max durations
fn stats(durations: &[Duration]) -> Stats {
    let mut sorted = durations.to_vec();
    sorted.sort();
    let min = *sorted.first().unwrap();
    let max = *sorted.last().unwrap();
    let mean = sorted.iter().sum::<Duration>() / sorted.len() as u32;
    let p50 = sorted[sorted.len() / 2];
    let p99 = sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)];
    let stddev = {
        let mean_secs = mean.as_secs_f64();
        let variance = sorted
            .iter()
            .map(|&d| {
                let diff = d.as_secs_f64() - mean_secs;
                diff * diff
            })
            .sum::<f64>()
            / sorted.len() as f64;
        Duration::from_secs_f64(variance.sqrt())
    };
    Stats {
        p50,
        p99,
        mean,
        stddev,
        min,
        max,
    }
}

fn start_container(image: &str, publish: &str, env: &[String]) -> Result<String, MiniRunError> {
    let output = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-d")
        .arg("-p")
        .arg(publish)
        .args(env.iter().flat_map(|e| ["-e", e]))
        .arg(image)
        .output()?;
    if !output.status.success() {
        return Err(MiniRunError::Docker(format!(
            "Failed to start container: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn stop_container(container_id: &str) -> Result<(), MiniRunError> {
    let output = Command::new("docker")
        .arg("stop")
        .arg(container_id)
        .output()?;
    if !output.status.success() {
        return Err(MiniRunError::Docker(format!(
            "Failed to stop container: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

async fn wait_for_200(url: &str, timeout: Duration) -> Result<(), MiniRunError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()?;
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(resp) = client.get(url).send().await
            && resp.status() == reqwest::StatusCode::OK
        {
            return Ok(());
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(MiniRunError::Timeout {
        target: url.to_string(),
        secs: timeout.as_secs(),
    })
}

/// How long a fresh TCP connection must stay open (and may stay silent)
/// to count as the real server rather than docker-proxy
const TCP_PROBE_WINDOW: Duration = Duration::from_millis(200);

/// Decides whether a TCP probe reached the real service.
/// `read` is `None` when nothing arrived within `TCP_PROBE_WINDOW`.
fn tcp_probe_alive(read: Option<std::io::Result<usize>>) -> bool {
    match read {
        // open and silent: server waits for the client to speak (Postgres, Redis)
        None => true,
        // EOF: docker-proxy accepted, found nothing behind it, closed
        Some(Ok(0)) => false,
        // server spoke first (MySQL, SSH, SMTP greetings)
        Some(Ok(_)) => true,
        // reset and friends: same as EOF, just rougher
        Some(Err(_)) => false,
    }
}

/// Returns the moment of the connect that turned out to reach the real
/// server, so the probe window is not added to the measurement
async fn wait_for_tcp(addr: &str, timeout: Duration) -> Result<Instant, MiniRunError> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(Ok(mut stream)) =
            tokio::time::timeout(Duration::from_secs(1), TcpStream::connect(addr)).await
        {
            let connected_at = Instant::now();
            let mut buf = [0u8; 1];
            let read = tokio::time::timeout(TCP_PROBE_WINDOW, stream.read(&mut buf))
                .await
                .ok();
            if tcp_probe_alive(read) {
                return Ok(connected_at);
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(MiniRunError::Timeout {
        target: addr.to_string(),
        secs: timeout.as_secs(),
    })
}

async fn measure_once(
    image: &str,
    publish: &str,
    env: &[String],
    target: &WaitTarget,
    timeout: Duration,
) -> Result<Duration, MiniRunError> {
    let start = Instant::now();
    let container_id = start_container(image, publish, env)?;
    let ready_at = match target {
        WaitTarget::Http(url) => wait_for_200(url, timeout).await.map(|()| Instant::now()),
        WaitTarget::Tcp(addr) => wait_for_tcp(addr, timeout).await,
    };
    stop_container(&container_id)?;
    Ok(ready_at? - start)
}

async fn run() -> Result<(), MiniRunError> {
    let args = Cli::parse();
    let target = WaitTarget::from_cli(&args);
    eprintln!(
        "would run {} ({} iters, wait: {})",
        args.image, args.iters, target
    );

    let mut durations = Vec::new();
    for i in 0..args.iters {
        eprintln!("Iteration {}/{}", i + 1, args.iters);
        match measure_once(
            &args.image,
            &args.publish,
            &args.env,
            &target,
            Duration::from_secs(args.timeout),
        )
        .await
        {
            Ok(duration) => {
                eprintln!("Ready ({}) after {} ms", target, duration.as_millis());
                durations.push(duration);
            }
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    }

    // All values and total sum of durations
    if !durations.is_empty() {
        for duration in &durations {
            eprintln!("  {} ms", duration.as_millis());
        }

        let summary = stats(&durations);
        match args.output {
            OutputFormat::Human => {
                println!("cold start ({} runs):", durations.len());
                println!(" p50: {} ms", summary.p50.as_millis());
                println!(" p99: {} ms", summary.p99.as_millis());
                println!(
                    " mean ± stddev: {} ± {} ms",
                    summary.mean.as_millis(),
                    summary.stddev.as_millis()
                );
                println!(" min: {} ms", summary.min.as_millis());
                println!(" max: {} ms", summary.max.as_millis());
            }
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary.to_json(durations.len()))?
                );
            }
        }
    }
    if durations.len() < args.iters as usize {
        eprintln!(
            "Warning: Only {} out of {} iterations succeeded",
            durations.len(),
            args.iters
        );
        std::process::exit(1);
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn stats_on_five_samples() {
        let durations = vec![ms(20), ms(10), ms(40), ms(30), ms(50)];
        let summary = stats(&durations);
        assert_eq!(summary.min, ms(10));
        assert_eq!(summary.max, ms(50));
        assert_eq!(summary.p50, ms(30));
        assert_eq!(summary.p99, ms(50));
        assert_eq!(summary.mean, ms(30));
        assert_eq!(summary.stddev.as_millis(), 14);
    }

    #[test]
    fn stats_on_single_sample() {
        let durations = vec![ms(42)];
        let summary = stats(&durations);
        assert_eq!(summary.min, ms(42));
        assert_eq!(summary.max, ms(42));
        assert_eq!(summary.p50, ms(42));
        assert_eq!(summary.p99, ms(42));
        assert_eq!(summary.mean, ms(42));
        assert_eq!(summary.stddev.as_millis(), 0);
    }

    #[test]
    fn stats_percentiles_on_hundred_samples() {
        let durations = (1..=100).map(ms).collect::<Vec<_>>();
        let summary = stats(&durations);
        assert_eq!(summary.min, ms(1));
        assert_eq!(summary.max, ms(100));
        assert_eq!(summary.p50, ms(51));
        assert_eq!(summary.p99, ms(100));
    }

    #[test]
    fn tcp_probe_tells_proxy_from_real_server() {
        use std::io::{Error, ErrorKind};
        assert!(tcp_probe_alive(None), "silent open socket = live server");
        assert!(!tcp_probe_alive(Some(Ok(0))), "EOF = docker-proxy");
        assert!(tcp_probe_alive(Some(Ok(1))), "greeting = live server");
        assert!(
            !tcp_probe_alive(Some(Err(Error::from(ErrorKind::ConnectionReset)))),
            "reset = not ready"
        );
    }

    #[test]
    fn wait_url_and_wait_tcp_are_mutually_exclusive() {
        let both = Cli::try_parse_from([
            "minirun",
            "--image",
            "x",
            "--wait-url",
            "http://a",
            "--wait-tcp",
            "a:1",
        ]);
        assert!(both.is_err());
    }

    #[test]
    fn wait_tcp_alone_overrides_default_wait_url() {
        let args = Cli::try_parse_from(["minirun", "--image", "x", "--wait-tcp", "a:1"]).unwrap();
        assert!(matches!(WaitTarget::from_cli(&args), WaitTarget::Tcp(addr) if addr == "a:1"));
    }
}
