use clap::Parser;

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

fn main() {
    let args = Cli::parse();
    println!(
        "would run {} ({} iters, wait_url: {})",
        args.image, args.iters, args.wait_url
    );
}
