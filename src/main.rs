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

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    println!(
        "would run {} ({} iters, wait_url: {})",
        args.image, args.iters, args.wait_url
    );

    let start = Instant::now();
    let container_id = match start_container(&args.image, &args.publish) {
        Ok(container_id) => {
            println!("Started container: {}", container_id);
            container_id
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let result = wait_for_200(&args.wait_url, Duration::from_secs(60)).await;
    let duration_ms = start.elapsed().as_millis();
    stop_container(&container_id).unwrap_or_else(|e| eprintln!("{}", e));
    match result {
        Ok(()) => {
            println!(
                "Successfully received 200 OK from {} after {} ms",
                args.wait_url, duration_ms
            );
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
