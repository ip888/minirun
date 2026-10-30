use clap::Parser;
use std::process::Command;
use std::time::{Duration, Instant};

/// All the ways a minirun measurement can fail
#[derive(Debug, thiserror::Error)]
enum MiniRunError {
    #[error("docker: {0}")]
    Docker(String),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("timeout waiting for {url} after {secs}s")]
    Timeout { url: String, secs: u64 },
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

    /// HTTP URL to poll for readiness
    #[arg(long, default_value = "http://localhost:8080")]
    wait_url: String,

    /// Number of cold-start iterations to measure
    #[arg(long, default_value_t = 5)]
    iters: u32,

    /// Seconds to wait for the service to answer 200 OK per iteration
    #[arg(long, default_value_t = 60)]
    timeout: u64,

    /// Output format for the results
    #[arg(long, value_enum, default_value = "human")]
    output: OutputFormat,
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

fn start_container(image: &str, publish: &str) -> Result<String, MiniRunError> {
    let output = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-d")
        .arg("-p")
        .arg(publish)
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
        url: url.to_string(),
        secs: timeout.as_secs(),
    })
}

async fn measure_once(
    image: &str,
    publish: &str,
    wait_url: &str,
    timeout: Duration,
) -> Result<Duration, MiniRunError> {
    let start = Instant::now();
    let container_id = start_container(image, publish)?;
    let result = wait_for_200(wait_url, timeout).await;
    let elapsed = start.elapsed();
    stop_container(&container_id)?;
    result?;
    Ok(elapsed)
}

async fn run() -> Result<(), MiniRunError> {
    let args = Cli::parse();
    eprintln!(
        "would run {} ({} iters, wait_url: {})",
        args.image, args.iters, args.wait_url
    );

    let mut durations = Vec::new();
    for i in 0..args.iters {
        eprintln!("Iteration {}/{}", i + 1, args.iters);
        match measure_once(
            &args.image,
            &args.publish,
            &args.wait_url,
            Duration::from_secs(args.timeout),
        )
        .await
        {
            Ok(duration) => {
                eprintln!(
                    "Successfully received 200 OK from {} after {} ms",
                    args.wait_url,
                    duration.as_millis()
                );
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
