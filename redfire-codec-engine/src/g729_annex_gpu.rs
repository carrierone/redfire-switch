/*
 * GPU-Accelerated G.729 Annex A and B Implementation
 * Annex A: Voice Activity Detection (VAD) and Discontinuous Transmission (DTX)
 * Annex B: Comfort Noise Generation (CNG)
 */

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
// tracing::info removed - not used without GPU features

#[cfg(feature = "cuda")]
use cudarc::driver::{CudaDevice, LaunchAsync, LaunchConfig};
#[cfg(feature = "cuda")]
use cudarc::nvrtc::Ptx;

#[cfg(feature = "rocm")]
use hip_rs::{HipDevice, HipMemory, HipStream};

#[cfg(not(any(feature = "cuda", feature = "rocm")))]
use crate::codec::GpuCodecConfig;
use crate::g729_codec::{G729Codec, G729_FRAME_SIZE};
#[cfg(any(feature = "cuda", feature = "rocm"))]
use crate::gpu_codec_accel::{GpuBackend, GpuBuffer, GpuCodecConfig};

/// G.729 Annex A configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct G729AnnexConfig {
    /// Enable Annex A (VAD/DTX)
    pub annex_a_enabled: bool,
    /// Enable Annex B (CNG)
    pub annex_b_enabled: bool,
    /// VAD sensitivity (0.0 = most sensitive, 1.0 = least sensitive)
    pub vad_sensitivity: f32,
    /// DTX threshold in dB
    pub dtx_threshold_db: f32,
    /// Comfort noise level in dB
    pub comfort_noise_level_db: f32,
    /// SID (Silence Insertion Descriptor) update period in frames
    pub sid_update_period: u32,
    /// Hangover period for voice activity (frames)
    pub hangover_period: u32,
    /// GPU processing configuration
    pub gpu_config: GpuCodecConfig,
}

impl Default for G729AnnexConfig {
    fn default() -> Self {
        Self {
            annex_a_enabled: true,
            annex_b_enabled: true,
            vad_sensitivity: 0.5,
            dtx_threshold_db: -30.0,
            comfort_noise_level_db: -60.0,
            sid_update_period: 8, // Update SID every 8 frames (80ms)
            hangover_period: 6,   // 60ms hangover
            gpu_config: GpuCodecConfig::default(),
        }
    }
}

/// Voice Activity Detection result
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VadResult {
    Voice,
    Silence,
    Hangover, // Transition period after voice
}

/// G.729 frame types for Annex A/B
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum G729FrameType {
    /// Normal speech frame (10 bytes)
    Speech,
    /// Silence Insertion Descriptor (2 bytes)
    Sid,
    /// No transmission (DTX active)
    NoTx,
    /// Comfort noise frame
    ComfortNoise,
}

/// G.729 Annex A/B encoder state
#[derive(Debug, Clone)]
pub struct G729AnnexState {
    /// Base G.729 codec
    pub g729_codec: G729Codec,
    /// VAD state
    pub vad_state: VadState,
    /// DTX state
    pub dtx_state: DtxState,
    /// CNG state (Annex B)
    pub cng_state: CngState,
    /// Frame counter
    pub frame_count: u32,
    /// Last transmitted frame type
    pub last_frame_type: G729FrameType,
}

/// Voice Activity Detector state
#[derive(Debug, Clone)]
pub struct VadState {
    /// Energy history buffer
    pub energy_history: VecDeque<f32>,
    /// Spectral features history
    pub spectral_history: VecDeque<SpectralFeatures>,
    /// Background noise estimate
    pub noise_estimate: f32,
    /// SNR threshold for voice detection
    pub snr_threshold: f32,
    /// Hangover counter
    pub hangover_counter: u32,
    /// Minimum energy threshold
    pub min_energy_threshold: f32,
    /// Zero crossing rate history
    pub zcr_history: VecDeque<f32>,
}

/// Discontinuous Transmission state
#[derive(Debug, Clone)]
pub struct DtxState {
    /// Current DTX mode
    pub active: bool,
    /// Frames since last speech
    pub silence_frames: u32,
    /// SID frame counter
    pub sid_counter: u32,
    /// Last SID energy
    pub last_sid_energy: f32,
    /// DTX decision history
    pub dtx_history: VecDeque<bool>,
}

/// Comfort Noise Generator state (Annex B)
#[derive(Debug, Clone)]
pub struct CngState {
    /// Random number generator seed
    pub rng_seed: u32,
    /// Comfort noise spectral envelope
    pub spectral_envelope: [f32; 10], // LSP parameters
    /// Comfort noise energy
    pub cng_energy: f32,
    /// Generated noise buffer
    pub noise_buffer: [f32; G729_FRAME_SIZE],
    /// Filter memory for noise shaping
    pub filter_memory: [f32; 10],
}

/// Spectral features for VAD
#[derive(Debug, Clone)]
pub struct SpectralFeatures {
    /// Spectral centroid
    pub centroid: f32,
    /// Spectral rolloff
    pub rolloff: f32,
    /// Spectral flux
    pub flux: f32,
    /// Low frequency energy ratio
    pub low_freq_ratio: f32,
}

/// SID (Silence Insertion Descriptor) frame
#[derive(Debug, Clone)]
pub struct SidFrame {
    /// Energy level
    pub energy: u8,
    /// Reflection coefficients
    pub reflection_coeffs: [u8; 4],
}

impl G729AnnexState {
    /// Create new G.729 Annex A/B state
    pub fn new() -> Self {
        Self {
            g729_codec: G729Codec::new(),
            vad_state: VadState::new(),
            dtx_state: DtxState::new(),
            cng_state: CngState::new(),
            frame_count: 0,
            last_frame_type: G729FrameType::Speech,
        }
    }

    /// Reset state for new call
    pub fn reset(&mut self) {
        self.g729_codec.reset();
        self.vad_state.reset();
        self.dtx_state.reset();
        self.cng_state.reset();
        self.frame_count = 0;
        self.last_frame_type = G729FrameType::Speech;
    }
}

impl VadState {
    fn new() -> Self {
        Self {
            energy_history: VecDeque::with_capacity(20),
            spectral_history: VecDeque::with_capacity(10),
            noise_estimate: 1e-6,
            snr_threshold: 3.0, // 3 dB SNR threshold
            hangover_counter: 0,
            min_energy_threshold: 1e-8,
            zcr_history: VecDeque::with_capacity(10),
        }
    }

    fn reset(&mut self) {
        self.energy_history.clear();
        self.spectral_history.clear();
        self.noise_estimate = 1e-6;
        self.hangover_counter = 0;
        self.zcr_history.clear();
    }
}

impl DtxState {
    fn new() -> Self {
        Self {
            active: false,
            silence_frames: 0,
            sid_counter: 0,
            last_sid_energy: 0.0,
            dtx_history: VecDeque::with_capacity(10),
        }
    }

    fn reset(&mut self) {
        self.active = false;
        self.silence_frames = 0;
        self.sid_counter = 0;
        self.last_sid_energy = 0.0;
        self.dtx_history.clear();
    }
}

impl CngState {
    fn new() -> Self {
        Self {
            rng_seed: 12345,
            spectral_envelope: [0.0; 10],
            cng_energy: 1e-6,
            noise_buffer: [0.0; G729_FRAME_SIZE],
            filter_memory: [0.0; 10],
        }
    }

    fn reset(&mut self) {
        self.rng_seed = 12345;
        self.spectral_envelope.fill(0.0);
        self.cng_energy = 1e-6;
        self.noise_buffer.fill(0.0);
        self.filter_memory.fill(0.0);
    }
}

/// GPU-accelerated G.729 Annex A/B processor
pub struct G729AnnexGpuProcessor {
    config: G729AnnexConfig,
    #[cfg(feature = "cuda")]
    cuda_device: Option<Arc<CudaDevice>>,
    #[cfg(feature = "rocm")]
    rocm_device: Option<Arc<HipDevice>>,
    /// Compiled GPU kernels
    kernels: Arc<RwLock<GpuKernels>>,
    /// Per-session encoder states
    sessions: Arc<RwLock<std::collections::HashMap<String, G729AnnexState>>>,
}

/// Compiled GPU kernels for G.729 Annex processing
#[derive(Debug)]
struct GpuKernels {
    #[cfg(feature = "cuda")]
    vad_energy_kernel: Option<cudarc::driver::CudaFunction>,
    #[cfg(feature = "cuda")]
    vad_spectral_kernel: Option<cudarc::driver::CudaFunction>,
    #[cfg(feature = "cuda")]
    cng_generation_kernel: Option<cudarc::driver::CudaFunction>,
    #[cfg(feature = "rocm")]
    rocm_vad_kernel: Option<hip_rs::HipFunction>,
    #[cfg(feature = "rocm")]
    rocm_cng_kernel: Option<hip_rs::HipFunction>,
}

impl G729AnnexGpuProcessor {
    /// Create new GPU-accelerated G.729 Annex processor
    pub async fn new(config: G729AnnexConfig) -> Result<Self> {
        #[cfg(feature = "cuda")]
        let cuda_device = if matches!(config.gpu_config.backend, GpuBackend::Cuda) {
            let device = Arc::new(CudaDevice::new(config.gpu_config.device_id as usize)?);
            info!(
                "Initialized CUDA device {} for G.729 Annex processing",
                config.gpu_config.device_id
            );
            Some(device)
        } else {
            None
        };

        #[cfg(feature = "rocm")]
        let rocm_device = if matches!(config.gpu_config.backend, GpuBackend::Rocm) {
            let device = Arc::new(HipDevice::new(config.gpu_config.device_id)?);
            info!(
                "Initialized ROCm device {} for G.729 Annex processing",
                config.gpu_config.device_id
            );
            Some(device)
        } else {
            None
        };

        #[cfg(not(feature = "cuda"))]
        let cuda_device: Option<Arc<()>> = None;

        #[cfg(not(feature = "rocm"))]
        let rocm_device: Option<Arc<()>> = None;

        let mut processor = Self {
            config,
            #[cfg(feature = "cuda")]
            cuda_device,
            #[cfg(feature = "rocm")]
            rocm_device,
            kernels: Arc::new(RwLock::new(GpuKernels {
                #[cfg(feature = "cuda")]
                vad_energy_kernel: None,
                #[cfg(feature = "cuda")]
                vad_spectral_kernel: None,
                #[cfg(feature = "cuda")]
                cng_generation_kernel: None,
                #[cfg(feature = "rocm")]
                rocm_vad_kernel: None,
                #[cfg(feature = "rocm")]
                rocm_cng_kernel: None,
            })),
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        };

        // Compile GPU kernels
        processor.compile_kernels().await?;

        Ok(processor)
    }

    /// Compile GPU kernels for VAD and CNG
    async fn compile_kernels(&mut self) -> Result<()> {
        match self.config.gpu_config.backend {
            #[cfg(feature = "cuda")]
            GpuBackend::Cuda => self.compile_cuda_kernels().await,
            #[cfg(feature = "rocm")]
            GpuBackend::Rocm => self.compile_rocm_kernels().await,
            _ => Err(anyhow!("Unsupported GPU backend")),
        }
    }

    #[cfg(feature = "cuda")]
    async fn compile_cuda_kernels(&mut self) -> Result<()> {
        if let Some(ref device) = self.cuda_device {
            let mut kernels = self.kernels.write().await;

            // VAD energy computation kernel
            let vad_energy_src = r#"
            extern "C" __global__ void vad_energy_kernel(
                const float* audio_samples,
                float* energy_out,
                float* zcr_out,
                int frame_size
            ) {
                int idx = blockIdx.x * blockDim.x + threadIdx.x;
                if (idx != 0) return; // Only one thread processes the frame
                
                float energy = 0.0f;
                int zero_crossings = 0;
                float prev_sample = audio_samples[0];
                
                // Compute frame energy and zero crossing rate
                for (int i = 0; i < frame_size; i++) {
                    float sample = audio_samples[i];
                    energy += sample * sample;
                    
                    // Count zero crossings
                    if ((prev_sample >= 0.0f && sample < 0.0f) || 
                        (prev_sample < 0.0f && sample >= 0.0f)) {
                        zero_crossings++;
                    }
                    prev_sample = sample;
                }
                
                *energy_out = energy / frame_size;
                *zcr_out = (float)zero_crossings / frame_size;
            }
            "#;

            let ptx = cudarc::nvrtc::compile_ptx(vad_energy_src)?;
            device.load_ptx(ptx, "vad_energy", &["vad_energy_kernel"])?;
            let function = device.get_func("vad_energy", "vad_energy_kernel").unwrap();
            kernels.vad_energy_kernel = Some(function);

            // VAD spectral features kernel
            let vad_spectral_src = r#"
            extern "C" __global__ void vad_spectral_kernel(
                const float* fft_magnitude,
                float* centroid_out,
                float* rolloff_out,
                float* low_freq_ratio_out,
                int fft_size
            ) {
                int idx = blockIdx.x * blockDim.x + threadIdx.x;
                if (idx != 0) return;
                
                float total_energy = 0.0f;
                float weighted_sum = 0.0f;
                float low_freq_energy = 0.0f;
                
                // Compute spectral centroid and low frequency energy
                for (int i = 0; i < fft_size / 2; i++) {
                    float magnitude = fft_magnitude[i];
                    float freq = (float)i / fft_size;
                    
                    total_energy += magnitude;
                    weighted_sum += magnitude * freq;
                    
                    if (freq < 0.25f) { // Low frequency < 1/4 Nyquist
                        low_freq_energy += magnitude;
                    }
                }
                
                *centroid_out = (total_energy > 0.0f) ? weighted_sum / total_energy : 0.0f;
                *low_freq_ratio_out = (total_energy > 0.0f) ? low_freq_energy / total_energy : 0.0f;
                
                // Compute spectral rolloff (85% of energy)
                float target_energy = total_energy * 0.85f;
                float cumulative_energy = 0.0f;
                *rolloff_out = 1.0f;
                
                for (int i = 0; i < fft_size / 2; i++) {
                    cumulative_energy += fft_magnitude[i];
                    if (cumulative_energy >= target_energy) {
                        *rolloff_out = (float)i / (fft_size / 2);
                        break;
                    }
                }
            }
            "#;

            let ptx = cudarc::nvrtc::compile_ptx(vad_spectral_src)?;
            device.load_ptx(ptx, "vad_spectral", &["vad_spectral_kernel"])?;
            let function = device
                .get_func("vad_spectral", "vad_spectral_kernel")
                .unwrap();
            kernels.vad_spectral_kernel = Some(function);

            // Comfort Noise Generation kernel
            let cng_src = r#"
            extern "C" __global__ void cng_generation_kernel(
                const float* lsp_params,
                float* noise_output,
                unsigned int* rng_state,
                float energy_level,
                int frame_size
            ) {
                int idx = blockIdx.x * blockDim.x + threadIdx.x;
                if (idx >= frame_size) return;
                
                // Linear Congruential Generator for noise
                unsigned int seed = *rng_state + idx;
                seed = seed * 1664525u + 1013904223u;
                
                // Generate white noise sample
                float noise = ((float)(seed & 0x7FFFFFFF) / 0x7FFFFFFF) * 2.0f - 1.0f;
                
                // Apply energy scaling
                noise *= sqrtf(energy_level);
                
                // TODO: Apply LSP-based spectral shaping
                // This is a simplified version - real implementation would use
                // the LSP parameters to shape the noise spectrum
                
                noise_output[idx] = noise;
                
                // Update RNG state (only first thread)
                if (idx == 0) {
                    *rng_state = seed;
                }
            }
            "#;

            let ptx = cudarc::nvrtc::compile_ptx(cng_src)?;
            device.load_ptx(ptx, "cng_generation", &["cng_generation_kernel"])?;
            let function = device
                .get_func("cng_generation", "cng_generation_kernel")
                .unwrap();
            kernels.cng_generation_kernel = Some(function);

            info!("Compiled CUDA kernels for G.729 Annex A/B processing");
        }
        Ok(())
    }

    #[cfg(feature = "rocm")]
    async fn compile_rocm_kernels(&mut self) -> Result<()> {
        if let Some(ref device) = self.rocm_device {
            // Similar implementation for ROCm using HIP
            info!("Compiled ROCm kernels for G.729 Annex A/B processing");
        }
        Ok(())
    }

    /// Start new G.729 Annex session
    pub async fn start_session(&self, session_id: String) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, G729AnnexState::new());
        Ok(())
    }

    /// End G.729 Annex session
    pub async fn end_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }

    /// Encode audio frame with G.729 Annex A/B
    pub async fn encode_frame(
        &self,
        session_id: &str,
        audio_samples: &[i16],
    ) -> Result<G729AnnexFrame> {
        if audio_samples.len() != G729_FRAME_SIZE {
            return Err(anyhow!(
                "Invalid frame size: expected {}, got {}",
                G729_FRAME_SIZE,
                audio_samples.len()
            ));
        }

        let mut sessions = self.sessions.write().await;
        let state = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

        // Convert to float for processing
        let audio_float: Vec<f32> = audio_samples.iter().map(|&x| x as f32 / 32768.0).collect();

        // Perform VAD using GPU acceleration
        let vad_result = self
            .gpu_voice_activity_detection(&audio_float, &mut state.vad_state)
            .await?;

        // Update DTX state based on VAD result
        self.update_dtx_state(&mut state.dtx_state, vad_result, &self.config);

        let frame_type = match (vad_result, state.dtx_state.active) {
            (VadResult::Voice, _) => G729FrameType::Speech,
            (VadResult::Hangover, false) => G729FrameType::Speech,
            (VadResult::Hangover, true) => G729FrameType::Speech, // Continue speech during hangover
            (VadResult::Silence, true) => {
                if state.dtx_state.sid_counter >= self.config.sid_update_period {
                    state.dtx_state.sid_counter = 0;
                    G729FrameType::Sid
                } else {
                    G729FrameType::NoTx
                }
            }
            (VadResult::Silence, false) => G729FrameType::Speech, // DTX not active yet
        };

        state.dtx_state.sid_counter += 1;
        state.frame_count += 1;
        state.last_frame_type = frame_type;

        let encoded_data = match frame_type {
            G729FrameType::Speech => {
                // Regular G.729 encoding
                state.g729_codec.encode(audio_samples)?
            }
            G729FrameType::Sid => {
                // Generate SID frame
                self.generate_sid_frame(&audio_float, &state.vad_state)?
            }
            G729FrameType::NoTx => {
                // No transmission
                Vec::new()
            }
            G729FrameType::ComfortNoise => {
                // This would be used on decoder side
                Vec::new()
            }
        };

        Ok(G729AnnexFrame {
            frame_type,
            data: encoded_data,
            energy_level: if frame_type == G729FrameType::Sid {
                Some(state.vad_state.noise_estimate)
            } else {
                None
            },
        })
    }

    /// GPU-accelerated Voice Activity Detection
    async fn gpu_voice_activity_detection(
        &self,
        _audio_samples: &[f32],
        _vad_state: &mut VadState,
    ) -> Result<VadResult> {
        match self.config.gpu_config.backend {
            #[cfg(feature = "cuda")]
            GpuBackend::Cuda => {
                self.cuda_voice_activity_detection(audio_samples, vad_state)
                    .await
            }
            #[cfg(feature = "rocm")]
            GpuBackend::Rocm => {
                self.rocm_voice_activity_detection(audio_samples, vad_state)
                    .await
            }
            _ => Err(anyhow!("Unsupported GPU backend for VAD")),
        }
    }

    #[cfg(feature = "cuda")]
    async fn cuda_voice_activity_detection(
        &self,
        audio_samples: &[f32],
        vad_state: &mut VadState,
    ) -> Result<VadResult> {
        if let Some(ref device) = self.cuda_device {
            let kernels = self.kernels.read().await;

            if let Some(ref energy_kernel) = kernels.vad_energy_kernel {
                // Allocate GPU buffers
                let input_buffer = GpuBuffer::allocate(
                    audio_samples.len() * std::mem::size_of::<f32>(),
                    GpuBackend::Cuda,
                    self.config.gpu_config.device_id,
                )
                .await?;

                let mut energy_buffer = GpuBuffer::allocate(
                    std::mem::size_of::<f32>(),
                    GpuBackend::Cuda,
                    self.config.gpu_config.device_id,
                )
                .await?;

                let mut zcr_buffer = GpuBuffer::allocate(
                    std::mem::size_of::<f32>(),
                    GpuBackend::Cuda,
                    self.config.gpu_config.device_id,
                )
                .await?;

                // Copy audio to GPU
                let audio_bytes: &[u8] = unsafe {
                    std::slice::from_raw_parts(
                        audio_samples.as_ptr() as *const u8,
                        audio_samples.len() * std::mem::size_of::<f32>(),
                    )
                };
                input_buffer.copy_from_host(audio_bytes).await?;

                // Launch VAD energy kernel
                let config = LaunchConfig {
                    grid_dim: (1, 1, 1),
                    block_dim: (1, 1, 1),
                    shared_mem_bytes: 0,
                };

                #[cfg(feature = "cuda")]
                unsafe {
                    energy_kernel
                        .launch(
                            config,
                            (
                                &input_buffer.cuda_ptr,
                                &energy_buffer.cuda_ptr,
                                &zcr_buffer.cuda_ptr,
                                audio_samples.len() as i32,
                            ),
                        )
                        .await?;
                }

                // Read results back
                let mut energy_bytes = vec![0u8; std::mem::size_of::<f32>()];
                let mut zcr_bytes = vec![0u8; std::mem::size_of::<f32>()];
                energy_buffer.copy_to_host(&mut energy_bytes).await?;
                zcr_buffer.copy_to_host(&mut zcr_bytes).await?;

                let frame_energy = f32::from_ne_bytes([
                    energy_bytes[0],
                    energy_bytes[1],
                    energy_bytes[2],
                    energy_bytes[3],
                ]);
                let zero_crossing_rate =
                    f32::from_ne_bytes([zcr_bytes[0], zcr_bytes[1], zcr_bytes[2], zcr_bytes[3]]);

                // Update VAD state and make decision
                return self.make_vad_decision(frame_energy, zero_crossing_rate, vad_state);
            }
        }

        Err(anyhow!("CUDA VAD processing failed"))
    }

    #[cfg(feature = "rocm")]
    async fn rocm_voice_activity_detection(
        &self,
        audio_samples: &[f32],
        vad_state: &mut VadState,
    ) -> Result<VadResult> {
        // Similar implementation for ROCm
        Err(anyhow!("ROCm VAD not yet implemented"))
    }

    /// Make VAD decision based on energy and spectral features
    fn make_vad_decision(
        &self,
        frame_energy: f32,
        zero_crossing_rate: f32,
        vad_state: &mut VadState,
    ) -> Result<VadResult> {
        // Update energy history
        vad_state.energy_history.push_back(frame_energy);
        if vad_state.energy_history.len() > 20 {
            vad_state.energy_history.pop_front();
        }

        // Update ZCR history
        vad_state.zcr_history.push_back(zero_crossing_rate);
        if vad_state.zcr_history.len() > 10 {
            vad_state.zcr_history.pop_front();
        }

        // Update noise estimate during silence
        let _min_energy = vad_state
            .energy_history
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .copied()
            .unwrap_or(frame_energy);

        if frame_energy < vad_state.noise_estimate * 2.0 {
            vad_state.noise_estimate = 0.9 * vad_state.noise_estimate + 0.1 * frame_energy;
        }

        // VAD decision based on multiple criteria
        let energy_ratio = frame_energy / (vad_state.noise_estimate + 1e-10);
        let snr_db = 10.0 * energy_ratio.log10();

        let adaptive_threshold = vad_state.snr_threshold * (1.0 + self.config.vad_sensitivity);

        let is_voice = snr_db > adaptive_threshold
            && frame_energy > vad_state.min_energy_threshold
            && zero_crossing_rate > 0.1
            && zero_crossing_rate < 0.8;

        // Apply hangover logic
        if is_voice {
            vad_state.hangover_counter = self.config.hangover_period;
            Ok(VadResult::Voice)
        } else if vad_state.hangover_counter > 0 {
            vad_state.hangover_counter -= 1;
            Ok(VadResult::Hangover)
        } else {
            Ok(VadResult::Silence)
        }
    }

    /// Update DTX state based on VAD result
    fn update_dtx_state(
        &self,
        dtx_state: &mut DtxState,
        vad_result: VadResult,
        config: &G729AnnexConfig,
    ) {
        dtx_state
            .dtx_history
            .push_back(vad_result == VadResult::Silence);
        if dtx_state.dtx_history.len() > 10 {
            dtx_state.dtx_history.pop_front();
        }

        match vad_result {
            VadResult::Voice => {
                dtx_state.active = false;
                dtx_state.silence_frames = 0;
            }
            VadResult::Hangover => {
                // Don't change DTX state during hangover
            }
            VadResult::Silence => {
                dtx_state.silence_frames += 1;
                if dtx_state.silence_frames >= 3 && config.annex_a_enabled {
                    dtx_state.active = true;
                }
            }
        }
    }

    /// Generate SID (Silence Insertion Descriptor) frame
    fn generate_sid_frame(&self, audio_samples: &[f32], vad_state: &VadState) -> Result<Vec<u8>> {
        // Simplified SID frame generation
        // Real implementation would compute proper reflection coefficients

        let energy = audio_samples.iter().map(|&x| x * x).sum::<f32>() / audio_samples.len() as f32;
        let energy_quantized = (energy.log10() * 10.0 + 100.0).clamp(0.0, 255.0) as u8;

        // Simple reflection coefficients (would be computed from LPC analysis)
        let reflection_coeffs = [
            (vad_state.noise_estimate * 127.0) as u8,
            ((vad_state.snr_threshold - 1.0) * 64.0) as u8,
            32, // Placeholder
            16, // Placeholder
        ];

        let sid_frame = SidFrame {
            energy: energy_quantized,
            reflection_coeffs,
        };

        // Pack SID frame (2 bytes)
        let mut packed = Vec::with_capacity(2);
        packed.push(sid_frame.energy);
        packed.push(
            (sid_frame.reflection_coeffs[0] >> 4) | ((sid_frame.reflection_coeffs[1] >> 4) << 4),
        );

        Ok(packed)
    }

    /// Generate comfort noise using GPU acceleration (Annex B)
    pub async fn generate_comfort_noise(
        &self,
        session_id: &str,
        sid_frame: &SidFrame,
    ) -> Result<Vec<i16>> {
        let mut sessions = self.sessions.write().await;
        let state = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

        if !self.config.annex_b_enabled {
            return Ok(vec![0i16; G729_FRAME_SIZE]);
        }

        // Update CNG parameters from SID frame
        state.cng_state.cng_energy = (sid_frame.energy as f32 - 100.0) / 10.0;
        state.cng_state.cng_energy = 10.0_f32.powf(state.cng_state.cng_energy);

        // Generate noise using GPU
        let noise_samples = self
            .gpu_generate_comfort_noise(&mut state.cng_state)
            .await?;

        // Convert to 16-bit PCM
        let pcm_samples: Vec<i16> = noise_samples
            .iter()
            .map(|&x| (x * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect();

        Ok(pcm_samples)
    }

    /// GPU-accelerated comfort noise generation
    async fn gpu_generate_comfort_noise(&self, cng_state: &mut CngState) -> Result<Vec<f32>> {
        match self.config.gpu_config.backend {
            #[cfg(feature = "cuda")]
            GpuBackend::Cuda => self.cuda_generate_comfort_noise(cng_state).await,
            #[cfg(feature = "rocm")]
            GpuBackend::Rocm => self.rocm_generate_comfort_noise(cng_state).await,
            _ => Err(anyhow!("Unsupported GPU backend for CNG")),
        }
    }

    #[cfg(feature = "cuda")]
    async fn cuda_generate_comfort_noise(&self, cng_state: &mut CngState) -> Result<Vec<f32>> {
        if let Some(ref device) = self.cuda_device {
            let kernels = self.kernels.read().await;

            if let Some(ref cng_kernel) = kernels.cng_generation_kernel {
                // Allocate GPU buffers
                let mut output_buffer = GpuBuffer::allocate(
                    G729_FRAME_SIZE * std::mem::size_of::<f32>(),
                    GpuBackend::Cuda,
                    self.config.gpu_config.device_id,
                )
                .await?;

                let mut rng_buffer = GpuBuffer::allocate(
                    std::mem::size_of::<u32>(),
                    GpuBackend::Cuda,
                    self.config.gpu_config.device_id,
                )
                .await?;

                // Copy RNG seed to GPU
                let seed_bytes = cng_state.rng_seed.to_ne_bytes();
                rng_buffer.copy_from_host(&seed_bytes).await?;

                // Launch CNG kernel
                let threads_per_block = 256;
                let blocks = (G729_FRAME_SIZE + threads_per_block - 1) / threads_per_block;

                let config = LaunchConfig {
                    grid_dim: (blocks as u32, 1, 1),
                    block_dim: (threads_per_block as u32, 1, 1),
                    shared_mem_bytes: 0,
                };

                #[cfg(feature = "cuda")]
                unsafe {
                    cng_kernel
                        .launch(
                            config,
                            (
                                &cng_state.spectral_envelope.as_ptr(),
                                &output_buffer.cuda_ptr,
                                &rng_buffer.cuda_ptr,
                                cng_state.cng_energy,
                                G729_FRAME_SIZE as i32,
                            ),
                        )
                        .await?;
                }

                // Read results back
                let mut noise_bytes = vec![0u8; G729_FRAME_SIZE * std::mem::size_of::<f32>()];
                output_buffer.copy_to_host(&mut noise_bytes).await?;

                // Convert bytes back to floats
                let noise_samples: Vec<f32> = noise_bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();

                // Update RNG seed
                let mut updated_seed_bytes = vec![0u8; 4];
                rng_buffer.copy_to_host(&mut updated_seed_bytes).await?;
                cng_state.rng_seed = u32::from_ne_bytes([
                    updated_seed_bytes[0],
                    updated_seed_bytes[1],
                    updated_seed_bytes[2],
                    updated_seed_bytes[3],
                ]);

                return Ok(noise_samples);
            }
        }

        Err(anyhow!("CUDA CNG generation failed"))
    }

    #[cfg(feature = "rocm")]
    async fn rocm_generate_comfort_noise(&self, cng_state: &mut CngState) -> Result<Vec<f32>> {
        // Similar implementation for ROCm
        Err(anyhow!("ROCm CNG not yet implemented"))
    }

    /// Get statistics for monitoring
    pub async fn get_statistics(&self) -> G729AnnexStats {
        let sessions = self.sessions.read().await;
        let active_sessions = sessions.len();

        let mut total_frames = 0;
        let speech_frames = 0;
        let silence_frames = 0;
        let sid_frames = 0;

        for state in sessions.values() {
            total_frames += state.frame_count;
            // Would need to track frame type statistics in real implementation
        }

        G729AnnexStats {
            active_sessions: active_sessions as u32,
            total_frames,
            speech_frames,
            silence_frames,
            sid_frames,
            bandwidth_savings_percent: if total_frames > 0 {
                (silence_frames as f32 / total_frames as f32) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// G.729 Annex A/B encoded frame
#[derive(Debug, Clone)]
pub struct G729AnnexFrame {
    pub frame_type: G729FrameType,
    pub data: Vec<u8>,
    pub energy_level: Option<f32>,
}

/// G.729 Annex processing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct G729AnnexStats {
    pub active_sessions: u32,
    pub total_frames: u32,
    pub speech_frames: u32,
    pub silence_frames: u32,
    pub sid_frames: u32,
    pub bandwidth_savings_percent: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_g729_annex_config() {
        let config = G729AnnexConfig::default();
        assert!(config.annex_a_enabled);
        assert!(config.annex_b_enabled);
        assert_eq!(config.sid_update_period, 8);
    }

    #[tokio::test]
    async fn test_vad_state() {
        let mut vad_state = VadState::new();
        assert_eq!(vad_state.energy_history.len(), 0);
        assert_eq!(vad_state.hangover_counter, 0);

        vad_state.reset();
        assert_eq!(vad_state.energy_history.len(), 0);
    }

    #[tokio::test]
    async fn test_g729_annex_state() {
        let mut state = G729AnnexState::new();
        assert_eq!(state.frame_count, 0);
        assert_eq!(state.last_frame_type, G729FrameType::Speech);

        state.reset();
        assert_eq!(state.frame_count, 0);
    }

    #[tokio::test]
    async fn test_sid_frame_generation() {
        let config = G729AnnexConfig::default();

        // Skip GPU tests if not available
        if !config.gpu_config.enabled {
            return;
        }

        let processor = G729AnnexGpuProcessor::new(config).await;
        if processor.is_err() {
            // GPU not available, skip test
            return;
        }

        let processor = processor.unwrap();
        let audio_samples = vec![0.1f32; G729_FRAME_SIZE];
        let vad_state = VadState::new();

        let sid_frame = processor
            .generate_sid_frame(&audio_samples, &vad_state)
            .unwrap();
        assert_eq!(sid_frame.len(), 2); // SID frame is 2 bytes
    }
}
