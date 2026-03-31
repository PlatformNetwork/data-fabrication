use anyhow::{Context, Result};

/// Query and display the status of a submission.
///
/// # Arguments
/// * `submission_id` - The unique identifier of the submission
/// * `rpc_url` - The RPC endpoint URL
///
/// # Returns
/// Ok(()) on success, Err on failure
pub async fn status_command(submission_id: &str, rpc_url: &str) -> Result<()> {
    println!("\n  Data Fabrication Challenge - Submission Status\n");

    let client = reqwest::Client::new();

    let url = format!("{}/submission/{}", rpc_url.trim_end_matches('/'), submission_id);

    let resp = client
        .get(&url)
        .send()
        .await
        .context("Unable to connect to validator. Please check the RPC URL and try again.")?;

    let status_code = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);

    if !status_code.is_success() {
        let msg = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("Submission not found");
        println!("  {}\n", msg);
        return Ok(());
    }

    println!(
        "  Submission ID:  {}",
        body.get("submission_id")
            .and_then(|v| v.as_str())
            .unwrap_or(submission_id)
    );

    println!(
        "  Hotkey:         {}",
        body.get("miner_hotkey")
            .and_then(|v| v.as_str())
            .unwrap_or("-")
    );

    println!(
        "  Status:         {}",
        body.get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("-")
    );

    if let Some(progress) = body.get("progress") {
        println!();
        if let Some(total) = progress.get("total_tasks").and_then(|v| v.as_u64()) {
            println!("  Total Tasks:    {}", total);
        }
        if let Some(completed) = progress.get("completed_tasks").and_then(|v| v.as_u64()) {
            println!("  Completed:      {}", completed);
        }
        if let Some(pending) = progress.get("pending_tasks").and_then(|v| v.as_u64()) {
            println!("  Pending:        {}", pending);
        }
        if let Some(pct) = progress.get("percentage").and_then(|v| v.as_f64()) {
            println!("  Progress:       {:.1}%", pct);
        }
    }

    if let Some(score) = body.get("score").and_then(|v| v.as_f64()) {
        println!("\n  Final Score:     {:.4}", score);
    }

    if let Some(submitted_at) = body.get("submitted_at").and_then(|v| v.as_str()) {
        println!("\n  Submitted:      {}", submitted_at);
    }

    if let Some(evaluation_time) = body.get("evaluation_time_ms").and_then(|v| v.as_u64()) {
        println!("  Eval Time:      {} ms", evaluation_time);
    }

    println!();
    Ok(())
}
