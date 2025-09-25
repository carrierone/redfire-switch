//! Batch Transcoding Service
//!
//! This service manages batch processing of audio transcoding and transcription
//! to prevent CPU overload and optimize resource utilization.
//!
//! Key features:
//! - Batch processing with configurable limits
//! - CPU load monitoring and throttling
//! - Priority-based queue management
//! - Resource-aware scheduling
//! - Back-pressure handling

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn, instrument};

use crate::events::{EventBus, TelecomEvent};
use crate::services::audio_recording::{AudioTranscoder, RecordingCodec, RtpAudioPacket};
use crate::services::vosk_client::{TranscriptionRequest, VoskClientService};

/// Batch transcoding priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TranscodingPriority {
    /// Low priority - fraud detection sampling
    Low = 0,
    /// Normal priority - regular monitoring
    Normal = 1,
    /// High priority - legal authorization cases
    High = 2,
    /// Critical priority - emergency/court-ordered intercepts
    Critical = 3,
}

/// Batch transcoding job
#[derive(Debug, Clone)]
pub struct TranscodingJob {
    pub job_id: String,
    pub recording_id: String,
    pub call_id: String,
    pub priority: TranscodingPriority,
    pub input_codec: RecordingCodec,
    pub audio_packets: Vec<RtpAudioPacket>,
    pub legal_authorization_id: Option<i32>,
    pub submitted_at: DateTime<Utc>,
    pub max_processing_time_ms: u64,
}

/// Batch transcription job
#[derive(Debug, Clone)]
pub struct TranscriptionJob {
    pub job_id: String,
    pub recording_id: String,
    pub call_id: String,
    pub session_id: String,
    pub priority: TranscodingPriority,
    pub wav_audio_data: Vec<u8>,
    pub sample_rate: u32,
    pub legal_authorization_id: Option<i32>,
    pub submitted_at: DateTime<Utc>,
    pub max_processing_time_ms: u64,
}

/// CPU load monitoring metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuLoadMetrics {
    pub current_load_percent: f64,
    pub average_load_1min: f64,
    pub average_load_5min: f64,
    pub transcoding_jobs_active: usize,
    pub transcription_jobs_active: usize,
    pub queue_backlog: usize,
    pub last_updated: DateTime<Utc>,
}

/// Batch processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTranscodingConfig {
    /// Maximum concurrent transcoding jobs
    pub max_concurrent_transcoding_jobs: usize,
    /// Maximum concurrent transcription jobs
    pub max_concurrent_transcription_jobs: usize,
    /// Batch size for transcoding operations
    pub transcoding_batch_size: usize,
    /// Batch size for transcription operations
    pub transcription_batch_size: usize,
    /// CPU load threshold to throttle processing (0.0-1.0)
    pub cpu_throttle_threshold: f64,
    /// CPU load threshold to pause processing (0.0-1.0)
    pub cpu_pause_threshold: f64,
    /// Interval for CPU load monitoring (milliseconds)
    pub cpu_monitor_interval_ms: u64,
    /// Maximum queue size before rejecting new jobs
    pub max_queue_size: usize,
    /// Job timeout in milliseconds
    pub job_timeout_ms: u64,
    /// Batch processing interval (milliseconds)
    pub batch_interval_ms: u64,
    /// Enable priority queue processing
    pub enable_priority_processing: bool,
    /// Back-pressure delay when CPU is high (milliseconds)
    pub backpressure_delay_ms: u64,
}

impl Default for BatchTranscodingConfig {
    fn default() -> Self {
        Self {
            max_concurrent_transcoding_jobs: 4,
            max_concurrent_transcription_jobs: 2,
            transcoding_batch_size: 10,
            transcription_batch_size: 5,
            cpu_throttle_threshold: 0.75, // 75% CPU
            cpu_pause_threshold: 0.90,     // 90% CPU
            cpu_monitor_interval_ms: 1000, // 1 second
            max_queue_size: 1000,
            job_timeout_ms: 30000, // 30 seconds
            batch_interval_ms: 100, // 100ms batch intervals
            enable_priority_processing: true,
            backpressure_delay_ms: 500, // 500ms delay under load
        }
    }
}

/// Batch transcoding statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchTranscodingStats {
    pub total_jobs_processed: u64,
    pub total_jobs_failed: u64,
    pub total_jobs_timeout: u64,
    pub average_processing_time_ms: f64,
    pub current_queue_size: usize,
    pub peak_queue_size: usize,
    pub cpu_throttle_events: u64,
    pub cpu_pause_events: u64,
    pub jobs_by_priority: HashMap<TranscodingPriority, u64>,
}

/// Batch transcoding service
pub struct BatchTranscodingService {
    config: BatchTranscodingConfig,
    event_bus: Arc<EventBus>,
    vosk_client: Arc<VoskClientService>,

    // Job queues
    transcoding_queue: Arc<RwLock<VecDeque<TranscodingJob>>>,
    transcription_queue: Arc<RwLock<VecDeque<TranscriptionJob>>>,

    // Concurrency control
    transcoding_semaphore: Arc<Semaphore>,
    transcription_semaphore: Arc<Semaphore>,

    // Monitoring
    cpu_metrics: Arc<RwLock<CpuLoadMetrics>>,
    stats: Arc<RwLock<BatchTranscodingStats>>,

    // Job submission channels
    transcoding_sender: mpsc::UnboundedSender<TranscodingJob>,
    transcription_sender: mpsc::UnboundedSender<TranscriptionJob>,
}

impl BatchTranscodingService {
    /// Create new batch transcoding service
    pub fn new(
        config: BatchTranscodingConfig,
        event_bus: Arc<EventBus>,
        vosk_client: Arc<VoskClientService>,
    ) -> Result<Self> {
        let (transcoding_sender, transcoding_receiver) = mpsc::unbounded_channel();
        let (transcription_sender, transcription_receiver) = mpsc::unbounded_channel();

        let service = Self {
            transcoding_semaphore: Arc::new(Semaphore::new(config.max_concurrent_transcoding_jobs)),
            transcription_semaphore: Arc::new(Semaphore::new(config.max_concurrent_transcription_jobs)),
            config: config.clone(),
            event_bus: event_bus.clone(),
            vosk_client,
            transcoding_queue: Arc::new(RwLock::new(VecDeque::new())),
            transcription_queue: Arc::new(RwLock::new(VecDeque::new())),
            cpu_metrics: Arc::new(RwLock::new(CpuLoadMetrics::default())),
            stats: Arc::new(RwLock::new(BatchTranscodingStats::default())),
            transcoding_sender,
            transcription_sender,
        };

        // Start background processors
        service.start_job_receivers(transcoding_receiver, transcription_receiver);
        service.start_cpu_monitor();
        service.start_batch_processors();

        info!("Batch transcoding service initialized with {} transcoding / {} transcription workers",
              config.max_concurrent_transcoding_jobs, config.max_concurrent_transcription_jobs);

        Ok(service)
    }

    /// Submit transcoding job
    #[instrument(skip(self, job), fields(job_id = %job.job_id, priority = ?job.priority))]
    pub async fn submit_transcoding_job(&self, job: TranscodingJob) -> Result<()> {
        let current_queue_size = {
            let queue = self.transcoding_queue.read().await;
            queue.len()
        };

        if current_queue_size >= self.config.max_queue_size {
            warn!("Transcoding queue full, rejecting job: {}", job.job_id);
            return Err(anyhow::anyhow!("Transcoding queue is full"));
        }

        debug!("Submitting transcoding job: {} (priority: {:?})", job.job_id, job.priority);

        self.transcoding_sender.send(job)
            .context("Failed to submit transcoding job")?;

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.current_queue_size = current_queue_size + 1;
        if stats.current_queue_size > stats.peak_queue_size {
            stats.peak_queue_size = stats.current_queue_size;
        }

        Ok(())
    }

    /// Submit transcription job
    #[instrument(skip(self, job), fields(job_id = %job.job_id, priority = ?job.priority))]
    pub async fn submit_transcription_job(&self, job: TranscriptionJob) -> Result<()> {
        let current_queue_size = {
            let queue = self.transcription_queue.read().await;
            queue.len()
        };

        if current_queue_size >= self.config.max_queue_size {
            warn!("Transcription queue full, rejecting job: {}", job.job_id);
            return Err(anyhow::anyhow!("Transcription queue is full"));
        }

        debug!("Submitting transcription job: {} (priority: {:?})", job.job_id, job.priority);

        self.transcription_sender.send(job)
            .context("Failed to submit transcription job")?;

        Ok(())
    }

    /// Start job receiver tasks
    fn start_job_receivers(
        &self,
        mut transcoding_receiver: mpsc::UnboundedReceiver<TranscodingJob>,
        mut transcription_receiver: mpsc::UnboundedReceiver<TranscriptionJob>,
    ) {
        let transcoding_queue = self.transcoding_queue.clone();
        let transcription_queue = self.transcription_queue.clone();
        let config = self.config.clone();

        // Transcoding job receiver
        tokio::spawn(async move {
            while let Some(job) = transcoding_receiver.recv().await {
                let mut queue = transcoding_queue.write().await;

                if config.enable_priority_processing {
                    // Insert job based on priority
                    let insert_pos = queue
                        .iter()
                        .position(|existing| existing.priority < job.priority)
                        .unwrap_or(queue.len());
                    queue.insert(insert_pos, job);
                } else {
                    queue.push_back(job);
                }
            }
        });

        // Transcription job receiver
        tokio::spawn(async move {
            while let Some(job) = transcription_receiver.recv().await {
                let mut queue = transcription_queue.write().await;

                if config.enable_priority_processing {
                    // Insert job based on priority
                    let insert_pos = queue
                        .iter()
                        .position(|existing| existing.priority < job.priority)
                        .unwrap_or(queue.len());
                    queue.insert(insert_pos, job);
                } else {
                    queue.push_back(job);
                }
            }
        });
    }

    /// Start CPU monitoring task
    fn start_cpu_monitor(&self) {
        let cpu_metrics = self.cpu_metrics.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.cpu_monitor_interval_ms));

            loop {
                interval.tick().await;

                // Get system CPU load (simplified - in production use sysinfo or similar)
                let cpu_load = Self::get_system_cpu_load().await;

                let mut metrics = cpu_metrics.write().await;
                metrics.current_load_percent = cpu_load;
                metrics.last_updated = Utc::now();

                // Update rolling averages (simplified)
                metrics.average_load_1min = (metrics.average_load_1min * 0.9) + (cpu_load * 0.1);
                metrics.average_load_5min = (metrics.average_load_5min * 0.95) + (cpu_load * 0.05);

                // Update statistics on high CPU
                if cpu_load > config.cpu_throttle_threshold {
                    let mut stats_guard = stats.write().await;
                    stats_guard.cpu_throttle_events += 1;
                }

                if cpu_load > config.cpu_pause_threshold {
                    let mut stats_guard = stats.write().await;
                    stats_guard.cpu_pause_events += 1;
                }

                debug!("CPU load: {:.1}% (1min avg: {:.1}%, 5min avg: {:.1}%)",
                       cpu_load * 100.0, metrics.average_load_1min * 100.0, metrics.average_load_5min * 100.0);
            }
        });
    }

    /// Start batch processing tasks
    fn start_batch_processors(&self) {
        self.start_transcoding_processor();
        self.start_transcription_processor();
    }

    /// Start transcoding batch processor
    fn start_transcoding_processor(&self) {
        let queue = self.transcoding_queue.clone();
        let semaphore = self.transcoding_semaphore.clone();
        let cpu_metrics = self.cpu_metrics.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        let event_bus = self.event_bus.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.batch_interval_ms));

            loop {
                interval.tick().await;

                // Check CPU load
                let cpu_load = {
                    let metrics = cpu_metrics.read().await;
                    metrics.current_load_percent
                };

                if cpu_load > config.cpu_pause_threshold {
                    debug!("CPU load too high ({:.1}%), pausing transcoding", cpu_load * 100.0);
                    sleep(Duration::from_millis(config.backpressure_delay_ms)).await;
                    continue;
                }

                // Process batch
                let batch = {
                    let mut queue_guard = queue.write().await;
                    let batch_size = if cpu_load > config.cpu_throttle_threshold {
                        config.transcoding_batch_size / 2 // Reduce batch size under load
                    } else {
                        config.transcoding_batch_size
                    };

                    let actual_batch_size = batch_size.min(queue_guard.len());
                    (0..actual_batch_size)
                        .map(|_| queue_guard.pop_front().unwrap())
                        .collect::<Vec<_>>()
                };

                if batch.is_empty() {
                    continue;
                }

                info!("Processing transcoding batch of {} jobs (CPU: {:.1}%)",
                      batch.len(), cpu_load * 100.0);

                // Process jobs in parallel within the batch
                let semaphore_clone = semaphore.clone();
                let stats_clone = stats.clone();
                let event_bus_clone = event_bus.clone();
                let config_clone = config.clone();

                tokio::spawn(async move {
                    let mut handles = Vec::new();

                    for job in batch {
                        let semaphore_ref = semaphore_clone.clone();
                        let stats_ref = stats_clone.clone();
                        let event_bus_ref = event_bus_clone.clone();
                        let config_ref = config_clone.clone();

                        let handle = tokio::spawn(async move {
                            let permit = semaphore_ref.acquire().await.unwrap();
                            let _permit = permit; // Hold permit for duration of job

                            let start_time = Instant::now();
                            let result = Self::process_transcoding_job(job.clone()).await;
                            let processing_time = start_time.elapsed();

                            // Update statistics
                            let mut stats_guard = stats_ref.write().await;
                            if result.is_ok() {
                                stats_guard.total_jobs_processed += 1;
                            } else {
                                stats_guard.total_jobs_failed += 1;
                            }

                            // Update average processing time
                            let current_avg = stats_guard.average_processing_time_ms;
                            let new_time = processing_time.as_millis() as f64;
                            stats_guard.average_processing_time_ms =
                                (current_avg * 0.9) + (new_time * 0.1);

                            *stats_guard.jobs_by_priority.entry(job.priority).or_insert(0) += 1;

                            if let Err(e) = result {
                                error!("Transcoding job {} failed: {}", job.job_id, e);

                                // Emit failure event
                                let event = TelecomEvent::VoiceIntegrityAudit {
                                    user_id: None,
                                    action_type: "transcoding_job_failed".to_string(),
                                    resource_type: "transcoding_job".to_string(),
                                    resource_id: job.job_id,
                                    authorization_id: job.legal_authorization_id,
                                    ecpa_compliant: true,
                                };
                                let _ = event_bus_ref.publish(event).await;
                            }
                        });

                        handles.push(handle);
                    }

                    // Wait for all jobs in batch to complete
                    for handle in handles {
                        let _ = handle.await;
                    }
                });
            }
        });
    }

    /// Start transcription batch processor
    fn start_transcription_processor(&self) {
        let queue = self.transcription_queue.clone();
        let semaphore = self.transcription_semaphore.clone();
        let cpu_metrics = self.cpu_metrics.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        let vosk_client = self.vosk_client.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.batch_interval_ms * 2)); // Slower for transcription

            loop {
                interval.tick().await;

                // Check CPU load
                let cpu_load = {
                    let metrics = cpu_metrics.read().await;
                    metrics.current_load_percent
                };

                if cpu_load > config.cpu_pause_threshold {
                    debug!("CPU load too high ({:.1}%), pausing transcription", cpu_load * 100.0);
                    sleep(Duration::from_millis(config.backpressure_delay_ms)).await;
                    continue;
                }

                // Process batch
                let batch = {
                    let mut queue_guard = queue.write().await;
                    let batch_size = if cpu_load > config.cpu_throttle_threshold {
                        1 // Process one at a time under load
                    } else {
                        config.transcription_batch_size
                    };

                    let actual_batch_size = batch_size.min(queue_guard.len());
                    (0..actual_batch_size)
                        .map(|_| queue_guard.pop_front().unwrap())
                        .collect::<Vec<_>>()
                };

                if batch.is_empty() {
                    continue;
                }

                info!("Processing transcription batch of {} jobs (CPU: {:.1}%)",
                      batch.len(), cpu_load * 100.0);

                // Process transcription jobs sequentially to avoid overloading Vosk
                for job in batch {
                    let semaphore_clone = semaphore.clone();
                    let stats_ref = stats.clone();
                    let vosk_ref = vosk_client.clone();

                    tokio::spawn(async move {
                        let permit = semaphore_clone.acquire().await.unwrap();
                        let _permit = permit;

                        let start_time = Instant::now();
                        let result = Self::process_transcription_job(job.clone(), vosk_ref).await;
                        let processing_time = start_time.elapsed();

                        // Update statistics
                        let mut stats_guard = stats_ref.write().await;
                        if result.is_ok() {
                            stats_guard.total_jobs_processed += 1;
                        } else {
                            stats_guard.total_jobs_failed += 1;
                        }

                        let current_avg = stats_guard.average_processing_time_ms;
                        let new_time = processing_time.as_millis() as f64;
                        stats_guard.average_processing_time_ms =
                            (current_avg * 0.9) + (new_time * 0.1);

                        if let Err(e) = result {
                            error!("Transcription job {} failed: {}", job.job_id, e);
                        }
                    });
                }
            }
        });
    }

    /// Process a transcoding job
    async fn process_transcoding_job(job: TranscodingJob) -> Result<Vec<u8>> {
        debug!("Processing transcoding job: {} ({} packets)", job.job_id, job.audio_packets.len());

        // Create transcoder
        let wav_spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let transcoder = AudioTranscoder::new(job.input_codec, wav_spec);

        // Transcode all packets
        let mut all_samples = Vec::new();
        for packet in &job.audio_packets {
            let samples = transcoder.transcode_packet(&packet.payload)
                .context("Failed to transcode audio packet")?;
            all_samples.extend(samples);
        }

        // Create WAV data in memory
        let mut wav_data = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut wav_data);
            let mut wav_writer = hound::WavWriter::new(cursor, wav_spec)
                .context("Failed to create WAV writer")?;

            for sample in all_samples {
                wav_writer.write_sample(sample)
                    .context("Failed to write WAV sample")?;
            }

            wav_writer.finalize()
                .context("Failed to finalize WAV data")?;
        }

        debug!("Transcoding job {} completed: {} bytes WAV data", job.job_id, wav_data.len());
        Ok(wav_data)
    }

    /// Process a transcription job
    async fn process_transcription_job(
        job: TranscriptionJob,
        vosk_client: Arc<VoskClientService>,
    ) -> Result<()> {
        debug!("Processing transcription job: {} ({} bytes audio)", job.job_id, job.wav_audio_data.len());

        let transcription_request = TranscriptionRequest {
            recording_id: job.recording_id,
            call_id: job.call_id,
            session_id: job.session_id,
            audio_data: job.wav_audio_data,
            sample_rate: job.sample_rate,
            is_final: true,
            legal_authorization_id: job.legal_authorization_id,
        };

        vosk_client.transcribe_audio(transcription_request).await
            .context("Failed to submit transcription request")?;

        debug!("Transcription job {} completed", job.job_id);
        Ok(())
    }

    /// Get system CPU load (simplified implementation)
    async fn get_system_cpu_load() -> f64 {
        // In production, use sysinfo crate or read from /proc/loadavg
        // For now, return a simulated value
        use rand::prelude::*;
        let mut rng = thread_rng();
        rng.gen_range(0.1..0.8) // Simulate 10-80% CPU usage
    }

    /// Get current statistics
    pub async fn get_statistics(&self) -> BatchTranscodingStats {
        let stats = self.stats.read().await;
        let mut result = stats.clone();

        // Update current queue sizes
        let transcoding_queue_size = self.transcoding_queue.read().await.len();
        let transcription_queue_size = self.transcription_queue.read().await.len();
        result.current_queue_size = transcoding_queue_size + transcription_queue_size;

        result
    }

    /// Get current CPU metrics
    pub async fn get_cpu_metrics(&self) -> CpuLoadMetrics {
        let mut metrics = self.cpu_metrics.read().await.clone();

        // Update active job counts
        metrics.transcoding_jobs_active = self.config.max_concurrent_transcoding_jobs - self.transcoding_semaphore.available_permits();
        metrics.transcription_jobs_active = self.config.max_concurrent_transcription_jobs - self.transcription_semaphore.available_permits();
        metrics.queue_backlog = self.transcoding_queue.read().await.len() + self.transcription_queue.read().await.len();

        metrics
    }

    /// Update configuration
    pub async fn update_config(&mut self, new_config: BatchTranscodingConfig) {
        info!("Updating batch transcoding configuration");
        self.config = new_config;
    }

    /// Get queue status
    pub async fn get_queue_status(&self) -> HashMap<String, usize> {
        let mut status = HashMap::new();

        status.insert("transcoding_queue_size".to_string(), self.transcoding_queue.read().await.len());
        status.insert("transcription_queue_size".to_string(), self.transcription_queue.read().await.len());
        status.insert("transcoding_workers_available".to_string(), self.transcoding_semaphore.available_permits());
        status.insert("transcription_workers_available".to_string(), self.transcription_semaphore.available_permits());

        status
    }
}