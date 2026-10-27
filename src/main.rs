use clap::Parser;
use std::process::Command;
use std::time::{Duration, Instant};

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

fn start_container(image: &str, publish: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-d")
        .arg("-p")
        .arg(publish)
        .arg(image)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Failed to start container: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn stop_container(container_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("docker")
        .arg("stop")
        .arg(container_id)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Failed to stop container: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

async fn wait_for_200(url: &str, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
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
    Err(format!("Timeout waiting for 200 OK from {}", url).into())
}

async fn measure_once(
    image: &str,
    publish: &str,
    wait_url: &str,
) -> Result<Duration, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let container_id = start_container(image, publish)?;
    let result = wait_for_200(wait_url, Duration::from_secs(60)).await;
    let elapsed = start.elapsed();
    stop_container(&container_id)?;
    match result {
        Ok(()) => Ok(elapsed),
        Err(e) => Err(e),
    }
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    println!(
        "would run {} ({} iters, wait_url: {})",
        args.image, args.iters, args.wait_url
    );

    let mut durations = Vec::new();
    for i in 0..args.iters {
        println!("Iteration {}/{}", i + 1, args.iters);
        match measure_once(&args.image, &args.publish, &args.wait_url).await {
            Ok(duration) => {
                println!(
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
            println!("  {} ms", duration.as_millis());
        }

        let summary = stats(&durations);
        println!("Average duration: {} ms", summary.mean.as_millis());
        println!("Min duration: {} ms", summary.min.as_millis());
        println!("Max duration: {} ms", summary.max.as_millis());
        println!("P50 duration: {} ms", summary.p50.as_millis());
        println!("P99 duration: {} ms", summary.p99.as_millis());
        println!("Stddev duration: {} ms", summary.stddev.as_millis());
    }

    if durations.len() < args.iters as usize {
        eprintln!(
            "Warning: Only {} out of {} iterations succeeded",
            durations.len(),
            args.iters
        );
        std::process::exit(1);
    }
}
