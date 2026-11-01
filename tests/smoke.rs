use std::process::Command;

#[test]
fn timeout_is_reported_cleanly() {
    let output = Command::new(env!("CARGO_BIN_EXE_minirun"))
        .args([
            "--image",
            "nginx:latest",
            "--iters",
            "1",
            "--wait-url",
            "http://localhost:1",
            "--timeout",
            "2",
        ])
        .output()
        .expect("failed to run minirun binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit, stderr: {stderr}"
    );
    assert!(stderr.contains("timeout waiting for"), "stderr: {stderr}");
}
