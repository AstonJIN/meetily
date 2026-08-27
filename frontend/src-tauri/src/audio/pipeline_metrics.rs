//! Low-overhead, aggregated observations for the live audio pipeline.
//!
//! This module is intentionally observational. It does not decide whether a
//! queue may accept an item and it does not alter audio data or scheduling.
//! The bounded-queue migration in phase 1 will use these measurements as its
//! baseline.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use log::info;
use sysinfo::{get_current_pid, ProcessesToUpdate, System};

const REPORT_INTERVAL: Duration = Duration::from_secs(5);
const METRICS_ENV: &str = "MEETILY_AUDIO_PIPELINE_METRICS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineQueue {
    AudioInput,
    Transcription,
    Recording,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueueMetricsSnapshot {
    pub depth: u64,
    pub peak_depth: u64,
    pub enqueued: u64,
    pub dequeued: u64,
    pub send_failures: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioPipelineMetricsSnapshot {
    pub session_elapsed_ms: u64,
    pub input: QueueMetricsSnapshot,
    pub transcription: QueueMetricsSnapshot,
    pub recording: QueueMetricsSnapshot,
    pub processed_chunks: u64,
    pub average_processing_ms: f64,
    pub max_processing_ms: f64,
    pub input_oldest_wait_ms: Option<f64>,
    pub transcription_oldest_wait_ms: Option<f64>,
    pub recording_oldest_wait_ms: Option<f64>,
    pub rss_bytes: Option<u64>,
    pub rss_peak_bytes: Option<u64>,
    pub latest_audio_timestamp_s: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct QueueState {
    depth: u64,
    peak_depth: u64,
    enqueued: u64,
    dequeued: u64,
    send_failures: u64,
    oldest_enqueued_at: Option<Instant>,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            depth: 0,
            peak_depth: 0,
            enqueued: 0,
            dequeued: 0,
            send_failures: 0,
            oldest_enqueued_at: None,
        }
    }
}

#[derive(Debug, Default)]
struct QueueStates {
    input: QueueState,
    transcription: QueueState,
    recording: QueueState,
}

impl QueueStates {
    fn get_mut(&mut self, queue: PipelineQueue) -> &mut QueueState {
        match queue {
            PipelineQueue::AudioInput => &mut self.input,
            PipelineQueue::Transcription => &mut self.transcription,
            PipelineQueue::Recording => &mut self.recording,
        }
    }

    fn get(&self, queue: PipelineQueue) -> &QueueState {
        match queue {
            PipelineQueue::AudioInput => &self.input,
            PipelineQueue::Transcription => &self.transcription,
            PipelineQueue::Recording => &self.recording,
        }
    }
}

/// Shared session metrics. All values are aggregates; no per-chunk history is
/// retained, so the metrics themselves do not grow with meeting duration.
#[derive(Debug)]
pub struct AudioPipelineMetrics {
    enabled: bool,
    session_started_at: Mutex<Instant>,
    queues: Mutex<QueueStates>,
    processed_chunks: AtomicU64,
    processing_total_ns: AtomicU64,
    processing_max_ns: AtomicU64,
    rss_peak_bytes: AtomicU64,
    latest_audio_timestamp_bits: AtomicU64,
    has_latest_audio_timestamp: AtomicU64,
    last_report_ms: AtomicU64,
}

impl AudioPipelineMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            enabled: std::env::var(METRICS_ENV)
                .map(|value| !matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "false" | "off"))
                .unwrap_or(true),
            session_started_at: Mutex::new(Instant::now()),
            queues: Mutex::new(QueueStates::default()),
            processed_chunks: AtomicU64::new(0),
            processing_total_ns: AtomicU64::new(0),
            processing_max_ns: AtomicU64::new(0),
            rss_peak_bytes: AtomicU64::new(0),
            latest_audio_timestamp_bits: AtomicU64::new(0),
            has_latest_audio_timestamp: AtomicU64::new(0),
            last_report_ms: AtomicU64::new(0),
        })
    }

    /// Start a new observation window without changing any pipeline state.
    pub fn start_session(&self) {
        if !self.enabled {
            return;
        }
        if let Ok(mut started_at) = self.session_started_at.lock() {
            *started_at = Instant::now();
        }
        if let Ok(mut queues) = self.queues.lock() {
            *queues = QueueStates::default();
        }
        self.processed_chunks.store(0, Ordering::Relaxed);
        self.processing_total_ns.store(0, Ordering::Relaxed);
        self.processing_max_ns.store(0, Ordering::Relaxed);
        self.rss_peak_bytes.store(0, Ordering::Relaxed);
        self.latest_audio_timestamp_bits.store(0, Ordering::Relaxed);
        self.has_latest_audio_timestamp.store(0, Ordering::Relaxed);
        self.last_report_ms.store(0, Ordering::Relaxed);
    }

    pub fn enqueue(&self, queue: PipelineQueue) {
        if !self.enabled {
            return;
        }
        if let Ok(mut queues) = self.queues.lock() {
            let state = queues.get_mut(queue);
            state.depth = state.depth.saturating_add(1);
            state.peak_depth = state.peak_depth.max(state.depth);
            state.enqueued = state.enqueued.saturating_add(1);
            if state.oldest_enqueued_at.is_none() {
                state.oldest_enqueued_at = Some(Instant::now());
            }
        }
    }

    pub fn dequeue(&self, queue: PipelineQueue) {
        if !self.enabled {
            return;
        }
        if let Ok(mut queues) = self.queues.lock() {
            let state = queues.get_mut(queue);
            state.depth = state.depth.saturating_sub(1);
            state.dequeued = state.dequeued.saturating_add(1);
            if state.depth == 0 {
                state.oldest_enqueued_at = None;
            }
        }
    }

    pub fn send_failure(&self, queue: PipelineQueue) {
        if !self.enabled {
            return;
        }
        if let Ok(mut queues) = self.queues.lock() {
            let state = queues.get_mut(queue);
            state.send_failures = state.send_failures.saturating_add(1);
        }
    }

    pub fn observe_processed_chunk(&self, processing_time: Duration, audio_timestamp_s: f64) {
        if !self.enabled {
            return;
        }
        self.processed_chunks.fetch_add(1, Ordering::Relaxed);
        let processing_ns = processing_time.as_nanos().min(u64::MAX as u128) as u64;
        self.processing_total_ns.fetch_add(processing_ns, Ordering::Relaxed);
        update_max(&self.processing_max_ns, processing_ns);
        self.latest_audio_timestamp_bits
            .store(audio_timestamp_s.to_bits(), Ordering::Relaxed);
        self.has_latest_audio_timestamp.store(1, Ordering::Relaxed);
    }

    pub fn maybe_log_summary(&self, force: bool) {
        if !self.enabled {
            return;
        }
        let elapsed_ms = self.session_elapsed().as_millis().min(u64::MAX as u128) as u64;
        let last_report_ms = self.last_report_ms.load(Ordering::Relaxed);
        if !force && elapsed_ms.saturating_sub(last_report_ms) < REPORT_INTERVAL.as_millis() as u64 {
            return;
        }
        if !force
            && self
                .last_report_ms
                .compare_exchange(last_report_ms, elapsed_ms, Ordering::Relaxed, Ordering::Relaxed)
                .is_err()
        {
            return;
        }
        if force {
            self.last_report_ms.store(elapsed_ms, Ordering::Relaxed);
        }

        let snapshot = self.snapshot();
        info!(
            "audio_pipeline_metrics session_ms={} input_depth={}/{} input_wait_ms={:?} transcription_depth={}/{} recording_depth={}/{} processed={} avg_process_ms={:.2} max_process_ms={:.2} rss_bytes={:?} rss_peak_bytes={:?} latest_audio_ts_s={:?}",
            snapshot.session_elapsed_ms,
            snapshot.input.depth,
            snapshot.input.peak_depth,
            snapshot.input_oldest_wait_ms,
            snapshot.transcription.depth,
            snapshot.transcription.peak_depth,
            snapshot.recording.depth,
            snapshot.recording.peak_depth,
            snapshot.processed_chunks,
            snapshot.average_processing_ms,
            snapshot.max_processing_ms,
            snapshot.rss_bytes,
            snapshot.rss_peak_bytes,
            snapshot.latest_audio_timestamp_s,
        );
    }

    pub fn snapshot(&self) -> AudioPipelineMetricsSnapshot {
        if !self.enabled {
            return AudioPipelineMetricsSnapshot::default();
        }
        let (input, transcription, recording, input_wait, transcription_wait, recording_wait) =
            if let Ok(queues) = self.queues.lock() {
                (
                    queue_snapshot(queues.get(PipelineQueue::AudioInput)),
                    queue_snapshot(queues.get(PipelineQueue::Transcription)),
                    queue_snapshot(queues.get(PipelineQueue::Recording)),
                    oldest_wait_ms(queues.get(PipelineQueue::AudioInput)),
                    oldest_wait_ms(queues.get(PipelineQueue::Transcription)),
                    oldest_wait_ms(queues.get(PipelineQueue::Recording)),
                )
            } else {
                Default::default()
            };

        let processed_chunks = self.processed_chunks.load(Ordering::Relaxed);
        let total_processing_ns = self.processing_total_ns.load(Ordering::Relaxed);
        let average_processing_ms = if processed_chunks == 0 {
            0.0
        } else {
            total_processing_ns as f64 / processed_chunks as f64 / 1_000_000.0
        };
        let max_processing_ms = self.processing_max_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        let rss_bytes = current_process_rss_bytes();
        if let Some(rss) = rss_bytes {
            update_max(&self.rss_peak_bytes, rss);
        }
        let rss_peak_bytes = nonzero(self.rss_peak_bytes.load(Ordering::Relaxed));
        let latest_audio_timestamp_s = if self.has_latest_audio_timestamp.load(Ordering::Relaxed) == 1 {
            Some(f64::from_bits(
                self.latest_audio_timestamp_bits.load(Ordering::Relaxed),
            ))
        } else {
            None
        };

        AudioPipelineMetricsSnapshot {
            session_elapsed_ms: self.session_elapsed().as_millis().min(u64::MAX as u128) as u64,
            input,
            transcription,
            recording,
            processed_chunks,
            average_processing_ms,
            max_processing_ms,
            input_oldest_wait_ms: input_wait,
            transcription_oldest_wait_ms: transcription_wait,
            recording_oldest_wait_ms: recording_wait,
            rss_bytes,
            rss_peak_bytes,
            latest_audio_timestamp_s,
        }
    }

    fn session_elapsed(&self) -> Duration {
        self.session_started_at
            .lock()
            .map(|started_at| started_at.elapsed())
            .unwrap_or_default()
    }
}

fn queue_snapshot(state: &QueueState) -> QueueMetricsSnapshot {
    QueueMetricsSnapshot {
        depth: state.depth,
        peak_depth: state.peak_depth,
        enqueued: state.enqueued,
        dequeued: state.dequeued,
        send_failures: state.send_failures,
    }
}

fn oldest_wait_ms(state: &QueueState) -> Option<f64> {
    state
        .oldest_enqueued_at
        .map(|oldest| oldest.elapsed().as_secs_f64() * 1000.0)
}

fn update_max(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Relaxed);
    while value > current {
        match target.compare_exchange_weak(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

fn nonzero(value: u64) -> Option<u64> {
    (value != 0).then_some(value)
}

fn current_process_rss_bytes() -> Option<u64> {
    let pid = get_current_pid().ok()?;
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).map(|process| process.memory())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_queue_depth_and_peak_without_retaining_items() {
        let metrics = AudioPipelineMetrics::new();
        metrics.enqueue(PipelineQueue::AudioInput);
        metrics.enqueue(PipelineQueue::AudioInput);
        metrics.dequeue(PipelineQueue::AudioInput);
        metrics.send_failure(PipelineQueue::Transcription);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.input.depth, 1);
        assert_eq!(snapshot.input.peak_depth, 2);
        assert_eq!(snapshot.input.enqueued, 2);
        assert_eq!(snapshot.input.dequeued, 1);
        assert_eq!(snapshot.transcription.send_failures, 1);
    }

    #[test]
    fn records_processing_latency_and_audio_timestamp() {
        let metrics = AudioPipelineMetrics::new();
        metrics.observe_processed_chunk(Duration::from_millis(4), 1.25);
        metrics.observe_processed_chunk(Duration::from_millis(8), 2.5);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.processed_chunks, 2);
        assert!((snapshot.average_processing_ms - 6.0).abs() < f64::EPSILON);
        assert!((snapshot.max_processing_ms - 8.0).abs() < f64::EPSILON);
        assert_eq!(snapshot.latest_audio_timestamp_s, Some(2.5));
    }

    #[test]
    fn start_session_resets_observation_window() {
        let metrics = AudioPipelineMetrics::new();
        metrics.enqueue(PipelineQueue::Recording);
        metrics.observe_processed_chunk(Duration::from_millis(1), 3.0);
        metrics.start_session();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.recording, QueueMetricsSnapshot::default());
        assert_eq!(snapshot.processed_chunks, 0);
        assert_eq!(snapshot.latest_audio_timestamp_s, None);
    }
}
