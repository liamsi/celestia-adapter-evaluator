use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use sov_celestia_adapter::{CelestiaService, DaService};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;

/// Maximum number of concurrent in-flight blob submissions.
/// Memory budget: 1000 × 6 MiB = ~6 GB blob data, safe on 16 GB droplet.
const MAX_CONCURRENT_SUBMISSIONS: usize = 1_000;

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

pub async fn run_submission_loop(
    celestia_service: Arc<CelestiaService>,
    finish_time: Instant,
    result_tx: mpsc::UnboundedSender<SubmissionResult>,
    blob_size_min: usize,
    blob_size_max: usize,
) {
    tracing::info!(
        max_concurrent = MAX_CONCURRENT_SUBMISSIONS,
        "Starting submission loop"
    );
    let mut submission_tasks = JoinSet::new();
    let mut interval = tokio::time::interval(Duration::from_secs(6));
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_SUBMISSIONS));

    while Instant::now() < finish_time {
        interval.tick().await;

        // Wait for a permit before spawning a new task.
        // This blocks if we already have MAX_CONCURRENT_SUBMISSIONS in flight.
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let in_flight = MAX_CONCURRENT_SUBMISSIONS - semaphore.available_permits();
        tracing::info!(in_flight, "Kicking off new submission task");

        let service = celestia_service.clone();
        let tx = result_tx.clone();

        submission_tasks.spawn(async move {
            let result = submit_blob(&service, blob_size_min, blob_size_max).await;
            let _ = tx.send(result);
            // Permit is dropped here, releasing a slot for the next submission
            drop(permit);
        });

        // Clean up completed tasks to free memory
        while let Some(result) = submission_tasks.try_join_next() {
            if let Err(e) = result {
                tracing::warn!(error = %e, "Task panicked");
            }
        }
    }

    drop(result_tx);

    // Wait for remaining tasks to complete
    while submission_tasks.join_next().await.is_some() {}
}

fn generate_random_blob(blob_size_min: usize, blob_size_max: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(blob_size_min..=blob_size_max);
    let mut blob: Vec<u8> = vec![0u8; size];
    rng.fill(&mut blob[..]);
    blob
}

async fn submit_blob(
    celestia_service: &CelestiaService,
    blob_size_min: usize,
    blob_size_max: usize,
) -> anyhow::Result<(usize, Duration)> {
    let start = Instant::now();
    let blob = generate_random_blob(blob_size_min, blob_size_max);
    let receiver = celestia_service.send_transaction(&blob).await;
    let receipt = receiver.await??;
    let duration = start.elapsed();
    tracing::debug!(?receipt, ?duration, "Receipt from sov-celestia-adapter");
    Ok((blob.len(), duration))
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
            Err(e) => {
                stats.error_count += 1;
                tracing::info!(
                    error = %e,
                    total = stats.error_count,
                    "Submission failed",
                );
            }
        }
    }

    stats
}
