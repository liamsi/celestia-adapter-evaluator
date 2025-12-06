use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use sov_celestia_adapter::{CelestiaService, DaService};
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct Stats {
    pub success_count: u64,
    pub error_count: u64,
    pub successful_bytes: usize,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub total_duration_ms: u64,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            success_count: 0,
            error_count: 0,
            successful_bytes: 0,
            min_duration_ms: u64::MAX,
            max_duration_ms: 0,
            total_duration_ms: 0,
        }
    }
}

/// Result of a submission: (bytes_sent, duration)
pub type SubmissionResult = anyhow::Result<(usize, Duration)>;

/// Sequential submission loop - submits one blob at a time, waiting for confirmation
/// before sending the next. No worker pool, no intervals - maximum throughput per account.
pub async fn run_sequential_submission_loop(
    celestia_service: Arc<CelestiaService>,
    finish_time: Instant,
    result_tx: mpsc::UnboundedSender<SubmissionResult>,
    blob_size_min: usize,
    blob_size_max: usize,
) {
    tracing::info!(blob_size_min, blob_size_max, "Starting sequential submission loop");

    let mut submission_count = 0u64;

    while Instant::now() < finish_time {
        submission_count += 1;

        let blob = generate_random_blob(blob_size_min, blob_size_max);
        let blob_size = blob.len();

        tracing::info!(
            submission = submission_count,
            blob_size,
            "Submitting blob"
        );

        let start = Instant::now();

        // Submit and wait for confirmation
        let receiver = celestia_service.send_transaction(&blob).await;
        match receiver.await {
            Ok(Ok(receipt)) => {
                let duration = start.elapsed();
                tracing::info!(
                    submission = submission_count,
                    blob_size,
                    duration_ms = duration.as_millis(),
                    ?receipt,
                    "Submission succeeded"
                );
                let _ = result_tx.send(Ok((blob_size, duration)));
            }
            Ok(Err(e)) => {
                tracing::error!(
                    submission = submission_count,
                    error = %e,
                    "Submission failed"
                );
                let _ = result_tx.send(Err(e));
            }
            Err(e) => {
                tracing::error!(
                    submission = submission_count,
                    error = %e,
                    "Submission channel error"
                );
                let _ = result_tx.send(Err(anyhow::anyhow!("Channel error: {}", e)));
            }
        }
    }

    drop(result_tx);
}

fn generate_random_blob(blob_size_min: usize, blob_size_max: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(blob_size_min..=blob_size_max);
    let mut blob: Vec<u8> = vec![0u8; size];
    rng.fill(&mut blob[..]);
    blob
}

pub async fn run_stats_collector(
    mut result_rx: mpsc::UnboundedReceiver<SubmissionResult>,
) -> Stats {
    let mut stats = Stats::default();

    while let Some(result) = result_rx.recv().await {
        match result {
            Ok((bytes_sent, duration)) => {
                stats.success_count += 1;
                stats.successful_bytes += bytes_sent;

                let duration_ms = duration.as_millis() as u64;
                stats.min_duration_ms = stats.min_duration_ms.min(duration_ms);
                stats.max_duration_ms = stats.max_duration_ms.max(duration_ms);
                stats.total_duration_ms += duration_ms;

                tracing::info!(
                    total = stats.success_count,
                    duration_ms,
                    "Submission succeeded"
                );
            }
            Err(error) => {
                stats.error_count += 1;
                tracing::info!(?error, total = stats.error_count, "Submission failed");
            }
        }
    }

    stats
}
