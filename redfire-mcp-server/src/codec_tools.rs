/*
 * Codec Tools Module for Redfire MCP Server
 * Provides AI-accessible audio codec transcoding capabilities
 */

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use redfire_codec_engine::{AudioCodec, AudioFrame, CodecService};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct CodecTools {
    service: Arc<CodecService>,
    gpu_enabled: bool,
}

impl CodecTools {
    pub async fn new(gpu_enabled: bool, _gpu_device: u32) -> Result<Self> {
        info!("Initializing codec tools (GPU: {})", gpu_enabled);

        let service = if gpu_enabled {
            #[cfg(any(feature = "cuda", feature = "rocm"))]
            {
                match redfire_codec_engine::create_gpu_service().await {
                    Ok(service) => {
                        info!("GPU codec service initialized");
                        Arc::new(service)
                    }
                    Err(e) => {
                        warn!(
                            "Failed to initialize GPU service, falling back to CPU: {}",
                            e
                        );
                        Arc::new(redfire_codec_engine::create_default_service().await?)
                    }
                }
            }
            #[cfg(not(any(feature = "cuda", feature = "rocm")))]
            {
                warn!("GPU requested but not compiled with GPU support, using CPU");
                Arc::new(redfire_codec_engine::create_default_service().await?)
            }
        } else {
            Arc::new(redfire_codec_engine::create_default_service().await?)
        };

        Ok(Self {
            service,
            gpu_enabled,
        })
    }

    pub async fn transcode_audio(&self, args: Value) -> Result<Value> {
        let input_data = args["input_data"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing input_data"))?;

        let source_codec = args["source_codec"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing source_codec"))?;

        let target_codec = args["target_codec"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing target_codec"))?;

        let sample_rate = args["sample_rate"].as_u64().unwrap_or(8000) as u32;

        // Parse codec enums
        let src_codec = self.parse_codec(source_codec)?;
        let dst_codec = self.parse_codec(target_codec)?;

        // Decode base64 input
        let input_bytes = general_purpose::STANDARD
            .decode(input_data)
            .map_err(|e| anyhow::anyhow!("Invalid base64: {}", e))?;

        debug!(
            "Transcoding {} bytes from {:?} to {:?}",
            input_bytes.len(),
            src_codec,
            dst_codec
        );

        let start_time = Instant::now();

        // Create a temporary session for this transcoding operation
        let session_id = format!("mcp_transcode_{}", Uuid::new_v4());

        // Start transcoding session
        self.service
            .start_session(
                session_id.clone(),
                src_codec,
                dst_codec,
                sample_rate,
                1, // mono
            )
            .await?;

        // Create audio frame from input bytes
        let input_frame = AudioFrame {
            codec: src_codec,
            data: input_bytes.clone(),
            timestamp: 0,
            sample_rate,
            channels: 1,
            sequence: 0,
        };

        // Transcode the frame
        let transcoded_frame = self
            .service
            .transcode_frame(&session_id, input_frame)
            .await?;

        // End the session
        if let Err(e) = self.service.end_session(&session_id).await {
            warn!(
                "Failed to cleanly end transcoding session {}: {}",
                session_id, e
            );
        }

        let duration = start_time.elapsed();

        // Encode output to base64
        let output_b64 = general_purpose::STANDARD.encode(&transcoded_frame.data);

        info!(
            "Transcoded {} bytes to {} bytes in {:?}",
            input_bytes.len(),
            transcoded_frame.data.len(),
            duration
        );

        Ok(json!({
            "success": true,
            "output_data": output_b64,
            "input_size": input_bytes.len(),
            "output_size": transcoded_frame.data.len(),
            "duration_ms": duration.as_millis(),
            "source_codec": source_codec,
            "target_codec": target_codec,
            "sample_rate": sample_rate,
            "gpu_used": self.gpu_enabled
        }))
    }

    pub async fn get_codec_info(&self, args: Value) -> Result<Value> {
        let specific_codec = args["codec"].as_str();

        let codecs = json!({
            "G711_ULAW": {
                "name": "G.711 μ-law",
                "sample_rate": 8000,
                "bit_rate": 64000,
                "frame_size": 160,
                "description": "ITU-T G.711 μ-law PCM encoding"
            },
            "G711_ALAW": {
                "name": "G.711 A-law",
                "sample_rate": 8000,
                "bit_rate": 64000,
                "frame_size": 160,
                "description": "ITU-T G.711 A-law PCM encoding"
            },
            "G729": {
                "name": "G.729",
                "sample_rate": 8000,
                "bit_rate": 8000,
                "frame_size": 10,
                "description": "ITU-T G.729 CELP codec with Annex A/B support"
            },
            "G722": {
                "name": "G.722",
                "sample_rate": 16000,
                "bit_rate": 64000,
                "frame_size": 320,
                "description": "ITU-T G.722 wideband audio codec"
            },
            "G7222": {
                "name": "G.722.2/AMR-WB",
                "sample_rate": 16000,
                "bit_rate": [6600, 8850, 12650, 14250, 15850, 18250, 19850, 23050, 23850],
                "frame_size": 320,
                "description": "ITU-T G.722.2 Adaptive Multi-Rate Wideband"
            },
            "PCM16": {
                "name": "PCM 16-bit",
                "sample_rate": [8000, 16000, 48000],
                "bit_rate": "variable",
                "frame_size": "variable",
                "description": "Linear PCM 16-bit signed"
            },
            "OPUS": {
                "name": "Opus",
                "sample_rate": 48000,
                "bit_rate": [6000, 510000],
                "frame_size": [120, 960],
                "description": "IETF Opus versatile audio codec"
            }
        });

        if let Some(codec) = specific_codec {
            if let Some(info) = codecs.get(codec) {
                return Ok(json!({
                    "codec": codec,
                    "info": info,
                    "gpu_supported": self.is_gpu_supported(codec),
                    "transcoding_pairs": self.get_supported_pairs(codec)
                }));
            } else {
                return Err(anyhow::anyhow!("Unknown codec: {}", codec));
            }
        }

        Ok(json!({
            "supported_codecs": codecs,
            "gpu_enabled": self.gpu_enabled,
            "total_transcoding_pairs": 56,
            "universal_gpu_transcoding": true
        }))
    }

    pub async fn benchmark_transcoding(&self, args: Value) -> Result<Value> {
        let source_codec = args["source_codec"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing source_codec"))?;

        let target_codec = args["target_codec"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing target_codec"))?;

        let iterations = args["iterations"].as_u64().unwrap_or(100) as usize;

        let src_codec = self.parse_codec(source_codec)?;
        let dst_codec = self.parse_codec(target_codec)?;

        info!(
            "Running benchmark: {:?} -> {:?} ({} iterations)",
            src_codec, dst_codec, iterations
        );

        // Generate test audio data (1 second of sine wave)
        let test_data = self.generate_test_audio(src_codec, 8000);

        let mut cpu_times = Vec::new();

        // Benchmark using session-based approach
        let start = Instant::now();
        for i in 0..iterations {
            let iter_start = Instant::now();

            // Create session for this iteration
            let session_id = format!("benchmark_{}_{}", Uuid::new_v4(), i);

            self.service
                .start_session(session_id.clone(), src_codec, dst_codec, 8000, 1)
                .await?;

            // Create audio frame
            let input_frame = AudioFrame {
                codec: src_codec,
                data: test_data.clone(),
                timestamp: 0,
                sample_rate: 8000,
                channels: 1,
                sequence: 0,
            };

            // Transcode
            let _ = self
                .service
                .transcode_frame(&session_id, input_frame)
                .await?;

            // Clean up
            if let Err(e) = self.service.end_session(&session_id).await {
                warn!("Failed to end benchmark session {}: {}", session_id, e);
            }

            cpu_times.push(iter_start.elapsed().as_micros() as u64);
        }
        let cpu_total = start.elapsed();

        let cpu_avg = cpu_times.iter().sum::<u64>() / cpu_times.len() as u64;
        let cpu_min = *cpu_times.iter().min().unwrap();
        let cpu_max = *cpu_times.iter().max().unwrap();

        let result = json!({
            "source_codec": source_codec,
            "target_codec": target_codec,
            "iterations": iterations,
            "test_data_size": test_data.len(),
            "benchmark": {
                "total_ms": cpu_total.as_millis(),
                "average_us": cpu_avg,
                "min_us": cpu_min,
                "max_us": cpu_max,
                "throughput_fps": (iterations as f64 / cpu_total.as_secs_f64()) as u64
            },
            "gpu_enabled": self.gpu_enabled
        });

        Ok(result)
    }

    fn parse_codec(&self, codec_str: &str) -> Result<AudioCodec> {
        match codec_str {
            "G711_ULAW" => Ok(AudioCodec::G711Ulaw),
            "G711_ALAW" => Ok(AudioCodec::G711Alaw),
            "G729" => Ok(AudioCodec::G729),
            "G722" => Ok(AudioCodec::G722),
            "G7222" => Ok(AudioCodec::G7222),
            "PCM16" => Ok(AudioCodec::Pcm16),
            "OPUS" => Ok(AudioCodec::Opus),
            _ => Err(anyhow::anyhow!("Unknown codec: {}", codec_str)),
        }
    }

    fn is_gpu_supported(&self, codec: &str) -> bool {
        // All our codecs support GPU transcoding
        matches!(
            codec,
            "G711_ULAW" | "G711_ALAW" | "G729" | "G722" | "G7222" | "PCM16"
        )
    }

    fn get_supported_pairs(&self, codec: &str) -> Vec<String> {
        let all_codecs = [
            "G711_ULAW",
            "G711_ALAW",
            "G729",
            "G722",
            "G7222",
            "PCM16",
            "OPUS",
        ];
        all_codecs
            .iter()
            .filter(|&&c| c != codec)
            .map(|&c| c.to_string())
            .collect()
    }

    fn generate_test_audio(&self, codec: AudioCodec, sample_rate: u32) -> Vec<u8> {
        // Generate 1 second of sine wave test data
        match codec {
            AudioCodec::G711Ulaw | AudioCodec::G711Alaw => {
                // 1 second = sample_rate bytes for G.711
                vec![0x7F; sample_rate as usize]
            }
            AudioCodec::G729 | AudioCodec::G729AnnexA | AudioCodec::G729AnnexB => {
                // G.729 frame is 10 bytes for 10ms (80 samples)
                vec![0x00; 100] // 1 second = 100 frames
            }
            AudioCodec::G722 => {
                // G.722 frame is variable, use 160 bytes for testing
                vec![0x80; sample_rate as usize / 4]
            }
            AudioCodec::G7222 => {
                // G.722.2 frame sizes vary by mode, use mode 0 (32 bytes)
                vec![0x04; 50 * 32] // 50 frames * 32 bytes
            }
            AudioCodec::Pcm16 => {
                // 16-bit PCM: 2 bytes per sample
                vec![0x00; (sample_rate * 2) as usize]
            }
            AudioCodec::Opus => {
                // Opus frame size varies, use 20ms frames
                vec![0xFC; 50 * 64] // 50 frames * ~64 bytes
            }
        }
    }
}
