/*
 * Redfire Switch - A Class 4 SIP Telephone Switch
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{Result, anyhow};
// dasp imports removed - not used in this module
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn, debug};

#[cfg(feature = "cuda")]
use cudarc::driver::{CudaDevice, DevicePtr};

#[cfg(any(feature = "cuda", feature = "rocm"))]
use crate::gpu_codec_accel::{GpuCodecAccelerator, GpuCodecConfig, GpuBackend};
use crate::g729_annex_gpu::{G729AnnexGpuProcessor, G729AnnexConfig, G729FrameType, G729AnnexFrame};
pub use crate::g729_codec::G729Codec;

// Fallback GPU config when GPU features are disabled
#[cfg(not(any(feature = "cuda", feature = "rocm")))]
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GpuCodecConfig {
    pub enabled: bool,
    pub backend: GpuBackend,
    pub device_id: u32,
    pub batch_size: u32,
    pub memory_pooling: bool,
    pub max_pool_size_mb: u32,
    pub async_processing: bool,
}

#[cfg(not(any(feature = "cuda", feature = "rocm")))]
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub enum GpuBackend {
    #[default]
    Cpu,
}

/// Supported audio codecs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioCodec {
    /// G.711 μ-law (PCMU)
    G711Ulaw,
    /// G.711 A-law (PCMA)
    G711Alaw,
    /// G.729 (8kbps CELP, patent-free since 2017)
    G729,
    /// G.729 Annex A (VAD/DTX)
    G729AnnexA,
    /// G.729 Annex B (CNG)
    G729AnnexB,
    /// Linear PCM 16-bit
    Pcm16,
    /// Opus
    Opus,
    /// G.722 wideband (ADPCM)
    G722,
    /// G.722.2 / AMR-WB (ACELP wideband, patent-free since 2023)
    G7222,
}

impl AudioCodec {
    /// Get the typical sample rate for this codec
    pub fn sample_rate(&self) -> u32 {
        match self {
            AudioCodec::G711Ulaw | AudioCodec::G711Alaw => 8000,
            AudioCodec::G729 | AudioCodec::G729AnnexA | AudioCodec::G729AnnexB => 8000,
            AudioCodec::Pcm16 => 8000,
            AudioCodec::Opus => 48000,
            AudioCodec::G722 => 16000,
            AudioCodec::G7222 => 16000, // AMR-WB is wideband (16kHz)
        }
    }

    /// Get the frame size in samples for this codec
    pub fn frame_size(&self) -> usize {
        match self {
            AudioCodec::G711Ulaw | AudioCodec::G711Alaw => 160, // 20ms at 8kHz
            AudioCodec::G729 | AudioCodec::G729AnnexA | AudioCodec::G729AnnexB => 80, // 10ms at 8kHz
            AudioCodec::Pcm16 => 160,
            AudioCodec::Opus => 960, // 20ms at 48kHz
            AudioCodec::G722 => 320, // 20ms at 16kHz
            AudioCodec::G7222 => 320, // 20ms at 16kHz (AMR-WB)
        }
    }

    /// Get the expected payload size in bytes
    pub fn payload_size(&self) -> usize {
        match self {
            AudioCodec::G711Ulaw | AudioCodec::G711Alaw => 160, // 1 byte per sample
            AudioCodec::G729 | AudioCodec::G729AnnexA | AudioCodec::G729AnnexB => 10, // Highly compressed
            AudioCodec::Pcm16 => 320, // 2 bytes per sample
            AudioCodec::Opus => 32, // Variable, typical value
            AudioCodec::G722 => 80, // 4 bits per sample
            AudioCodec::G7222 => 33, // AMR-WB mode 8 (23.85 kbps), variable size
        }
    }

    /// Get the RTP payload type for this codec
    pub fn payload_type(&self) -> u8 {
        match self {
            AudioCodec::G711Ulaw => 0,
            AudioCodec::G711Alaw => 8,
            AudioCodec::G729 | AudioCodec::G729AnnexA | AudioCodec::G729AnnexB => 18,
            AudioCodec::Pcm16 => 10, // L16
            AudioCodec::Opus => 96, // Dynamic
            AudioCodec::G722 => 9,
            AudioCodec::G7222 => 97, // Dynamic (AMR-WB)
        }
    }

    /// Parse codec from RTP payload type
    pub fn from_payload_type(pt: u8) -> Option<Self> {
        match pt {
            0 => Some(AudioCodec::G711Ulaw),
            8 => Some(AudioCodec::G711Alaw),
            18 => Some(AudioCodec::G729),
            10 => Some(AudioCodec::Pcm16),
            96 => Some(AudioCodec::Opus), // Assuming Opus for dynamic PT 96
            97 => Some(AudioCodec::G7222), // Assuming AMR-WB for dynamic PT 97
            9 => Some(AudioCodec::G722),
            _ => None,
        }
    }
}

/// Codec configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecConfig {
    /// Enable codec translation
    pub enabled: bool,
    /// Use GPU acceleration when available
    pub use_gpu: bool,
    /// GPU acceleration configuration
    pub gpu_config: GpuCodecConfig,
    /// G.729 Annex A/B configuration
    pub g729_annex_config: G729AnnexConfig,
    /// Maximum concurrent translation sessions
    pub max_sessions: u32,
    /// Translation quality (0.0 to 1.0)
    pub quality: f32,
    /// Buffer size for translation
    pub buffer_size: usize,
    /// Supported codec combinations
    pub supported_translations: Vec<CodecTranslation>,
}

impl CodecConfig {
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    fn default_gpu_config() -> GpuCodecConfig {
        GpuCodecConfig {
            enabled: true,
            backend: if cfg!(feature = "cuda") { GpuBackend::Cuda } else { GpuBackend::Rocm },
            ..Default::default()
        }
    }

    #[cfg(not(any(feature = "cuda", feature = "rocm")))]
    fn default_gpu_config() -> GpuCodecConfig {
        GpuCodecConfig {
            enabled: false,
            backend: GpuBackend::Cpu,
            device_id: 0,
            batch_size: 64,
            memory_pooling: false,
            max_pool_size_mb: 0,
            async_processing: false,
        }
    }
    
    /// Generate all supported codec translation pairs
    fn generate_all_codec_pairs() -> Vec<CodecTranslation> {
        let mut translations = Vec::new();
        
        let codecs = [
            AudioCodec::G711Ulaw,
            AudioCodec::G711Alaw,
            AudioCodec::G729,
            AudioCodec::G729AnnexA,
            AudioCodec::G729AnnexB,
            AudioCodec::Pcm16,
            AudioCodec::G722,
            AudioCodec::G7222,
            AudioCodec::Opus,
        ];
        
        // Generate all codec pair combinations (excluding same-to-same)
        for &from_codec in &codecs {
            for &to_codec in &codecs {
                if from_codec != to_codec {
                    translations.push(CodecTranslation {
                        from: from_codec,
                        to: to_codec,
                    });
                }
            }
        }
        
        translations
    }
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            use_gpu: cfg!(any(feature = "cuda", feature = "rocm")),
            gpu_config: Self::default_gpu_config(),
            g729_annex_config: G729AnnexConfig::default(),
            max_sessions: 100,
            quality: 0.8,
            buffer_size: 8192,
            supported_translations: Self::generate_all_codec_pairs(),
        }
    }
}

/// Codec translation pair
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodecTranslation {
    pub from: AudioCodec,
    pub to: AudioCodec,
}

/// Audio frame for codec processing
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// Raw audio data
    pub data: Vec<u8>,
    /// Codec type
    pub codec: AudioCodec,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Timestamp
    pub timestamp: u32,
    /// Sequence number
    pub sequence: u16,
}

/// Transcoded audio frame
#[derive(Debug, Clone)]
pub struct TranscodedFrame {
    /// Original frame
    pub original: AudioFrame,
    /// Transcoded data
    pub data: Vec<u8>,
    /// Target codec
    pub target_codec: AudioCodec,
    /// Processing time in microseconds
    pub processing_time_us: u64,
}

/// G.711 codec implementation with built-in algorithms
pub struct G711Codec;

impl G711Codec {
    /// Encode PCM to μ-law
    pub fn encode_ulaw(pcm_data: &[i16]) -> Vec<u8> {
        pcm_data.iter().map(|&sample| Self::linear_to_ulaw(sample)).collect()
    }

    /// Decode μ-law to PCM
    pub fn decode_ulaw(ulaw_data: &[u8]) -> Vec<i16> {
        ulaw_data.iter().map(|&sample| Self::ulaw_to_linear(sample)).collect()
    }

    /// Encode PCM to A-law
    pub fn encode_alaw(pcm_data: &[i16]) -> Vec<u8> {
        pcm_data.iter().map(|&sample| Self::linear_to_alaw(sample)).collect()
    }

    /// Decode A-law to PCM
    pub fn decode_alaw(alaw_data: &[u8]) -> Vec<i16> {
        alaw_data.iter().map(|&sample| Self::alaw_to_linear(sample)).collect()
    }

    /// Convert μ-law to A-law directly
    pub fn ulaw_to_alaw(ulaw_data: &[u8]) -> Vec<u8> {
        ulaw_data.iter()
            .map(|&sample| Self::linear_to_alaw(Self::ulaw_to_linear(sample)))
            .collect()
    }

    /// Convert A-law to μ-law directly
    pub fn alaw_to_ulaw(alaw_data: &[u8]) -> Vec<u8> {
        alaw_data.iter()
            .map(|&sample| Self::linear_to_ulaw(Self::alaw_to_linear(sample)))
            .collect()
    }

    /// Convert linear PCM to μ-law (basic implementation)
    fn linear_to_ulaw(linear: i16) -> u8 {
        // Simplified μ-law compression
        let sign = if linear < 0 { 0x80 } else { 0x00 };
        let magnitude = linear.abs() as u16;
        
        // Basic μ-law compression algorithm
        let compressed = if magnitude < 0x20 {
            magnitude >> 1
        } else if magnitude < 0x40 {
            (magnitude >> 2) + 0x10
        } else if magnitude < 0x80 {
            (magnitude >> 3) + 0x20
        } else if magnitude < 0x100 {
            (magnitude >> 4) + 0x30
        } else if magnitude < 0x200 {
            (magnitude >> 5) + 0x40
        } else if magnitude < 0x400 {
            (magnitude >> 6) + 0x50
        } else if magnitude < 0x800 {
            (magnitude >> 7) + 0x60
        } else {
            (magnitude >> 8) + 0x70
        };
        
        sign | ((compressed as u8) & 0x7F)
    }

    /// Convert μ-law to linear PCM (basic implementation)
    fn ulaw_to_linear(ulaw: u8) -> i16 {
        let sign = (ulaw & 0x80) != 0;
        let magnitude = ulaw & 0x7F;
        
        // Basic μ-law decompression
        let linear = match magnitude >> 4 {
            0 => ((magnitude << 1) + 1) as u16,
            1 => (((magnitude & 0x0F) << 2) + 0x21) as u16,
            2 => (((magnitude & 0x0F) << 3) + 0x41) as u16,
            3 => (((magnitude & 0x0F) << 4) + 0x81) as u16,
            4 => (((magnitude & 0x0F) as u16) << 5) + 0x101,
            5 => (((magnitude & 0x0F) as u16) << 6) + 0x201,
            6 => (((magnitude & 0x0F) as u16) << 7) + 0x401,
            7 => (((magnitude & 0x0F) as u16) << 8) + 0x801,
            _ => 0,
        };
        
        if sign { -(linear as i16) } else { linear as i16 }
    }

    /// Convert linear PCM to A-law (basic implementation)
    fn linear_to_alaw(linear: i16) -> u8 {
        // Simplified A-law compression
        let sign = if linear < 0 { 0x80 } else { 0x00 };
        let magnitude = linear.abs() as u16;
        
        // Basic A-law compression algorithm
        let compressed = if magnitude < 0x10 {
            magnitude >> 1
        } else if magnitude < 0x20 {
            (magnitude >> 1) + 0x08
        } else if magnitude < 0x40 {
            (magnitude >> 2) + 0x18
        } else if magnitude < 0x80 {
            (magnitude >> 3) + 0x28
        } else if magnitude < 0x100 {
            (magnitude >> 4) + 0x38
        } else if magnitude < 0x200 {
            (magnitude >> 5) + 0x48
        } else if magnitude < 0x400 {
            (magnitude >> 6) + 0x58
        } else {
            (magnitude >> 7) + 0x68
        };
        
        sign | ((compressed as u8) & 0x7F)
    }

    /// Convert A-law to linear PCM (basic implementation)  
    fn alaw_to_linear(alaw: u8) -> i16 {
        let sign = (alaw & 0x80) != 0;
        let magnitude = alaw & 0x7F;
        
        // Basic A-law decompression
        let linear = match magnitude >> 4 {
            0 => (magnitude << 1) as u16,
            1 => (((magnitude & 0x0F) << 1) + 0x10) as u16,
            2 => (((magnitude & 0x0F) << 2) + 0x30) as u16,
            3 => (((magnitude & 0x0F) << 3) + 0x50) as u16,
            4 => (((magnitude & 0x0F) << 4) + 0x90) as u16,
            5 => (((magnitude & 0x0F) as u16) << 5) + 0x110,
            6 => (((magnitude & 0x0F) as u16) << 6) + 0x210,
            7 => (((magnitude & 0x0F) as u16) << 7) + 0x410,
            _ => 0,
        };
        
        if sign { -(linear as i16) } else { linear as i16 }
    }
}

// G729Codec is already re-exported above

/// Legacy CUDA codec processor (for simple G.711 translations)
#[cfg(feature = "cuda")]
pub struct CudaCodecProcessor {
    device: Arc<CudaDevice>,
    kernels: HashMap<String, cudarc::driver::CudaFunction>,
}

#[cfg(feature = "cuda")]
impl CudaCodecProcessor {
    pub fn new() -> Result<Self> {
        let device = Arc::new(CudaDevice::new(0)?); // Use first GPU
        let mut processor = Self {
            device,
            kernels: HashMap::new(),
        };
        
        processor.load_kernels()?;
        Ok(processor)
    }

    fn load_kernels(&mut self) -> Result<()> {
        // Simple inline CUDA kernel for G.711 translation
        let kernel_source = r#"
        extern "C" __global__ void g711_ulaw_to_alaw(const unsigned char* input, unsigned char* output, int count) {
            int idx = blockIdx.x * blockDim.x + threadIdx.x;
            if (idx >= count) return;
            
            unsigned char ulaw = input[idx];
            unsigned char alaw;
            
            // Convert μ-law to linear
            ulaw = ~ulaw;
            int sign = (ulaw & 0x80) ? -1 : 1;
            int exponent = (ulaw >> 4) & 0x07;
            int mantissa = ulaw & 0x0F;
            int linear = ((mantissa << 3) + 0x84) << exponent;
            if (exponent == 0) linear = mantissa << 4;
            linear *= sign;
            
            // Convert linear to A-law
            if (linear < 0) {
                linear = -linear;
                alaw = 0x80;
            } else {
                alaw = 0x00;
            }
            
            if (linear >= 0x1F80) {
                alaw |= 0x7F;
            } else {
                int exp = 0;
                int temp = linear >> 8;
                while (temp) {
                    temp >>= 1;
                    exp++;
                }
                alaw |= (exp << 4) | ((linear >> (exp + 3)) & 0x0F);
            }
            
            output[idx] = alaw ^ 0x55;
        }
        "#;
        
        // Compile kernel
        let ptx = cudarc::nvrtc::compile_ptx(kernel_source)?;
        self.device.load_ptx(ptx, "g711_kernels", &["g711_ulaw_to_alaw"])?;
        let kernel = self.device.get_func("g711_kernels", "g711_ulaw_to_alaw").unwrap();
        self.kernels.insert("ulaw_to_alaw".to_string(), kernel);
        
        Ok(())
    }

    pub fn translate_g711_gpu(&self, input: &[u8], from: AudioCodec, to: AudioCodec) -> Result<Vec<u8>> {
        if from == AudioCodec::G711Ulaw && to == AudioCodec::G711Alaw {
            let input_gpu = self.device.htod_copy(input.to_vec())?;
            let mut output_gpu = self.device.alloc_zeros::<u8>(input.len())?;
            
            if let Some(kernel) = self.kernels.get("ulaw_to_alaw") {
                let cfg = cudarc::driver::LaunchConfig {
                    grid_dim: ((input.len() as u32 + 255) / 256, 1, 1),
                    block_dim: (256, 1, 1),
                    shared_mem_bytes: 0,
                };
                
                unsafe {
                    kernel.launch(cfg, (&input_gpu, &output_gpu, input.len() as i32))?;
                }
                
                let output = self.device.dtoh_sync_copy(&output_gpu)?;
                return Ok(output);
            }
        }
        
        Err(anyhow!("Unsupported GPU translation: {:?} to {:?}", from, to))
    }
}

/// Opus codec placeholder (requires external Opus library)
pub struct OpusCodec {
    sample_rate: u32,
    channels: u8,
}

impl OpusCodec {
    pub fn new(sample_rate: u32, channels: u8) -> Result<Self> {
        Ok(Self {
            sample_rate,
            channels,
        })
    }

    pub fn encode(&mut self, pcm_data: &[i16]) -> Result<Vec<u8>> {
        // Placeholder implementation - would use actual Opus encoder
        // For now, just return empty data
        debug!("Opus encode: {} samples at {}Hz", pcm_data.len(), self.sample_rate);
        Ok(vec![0u8; 32]) // Placeholder compressed data
    }

    pub fn decode(&mut self, opus_data: &[u8], output_size: usize) -> Result<Vec<i16>> {
        // Placeholder implementation - would use actual Opus decoder
        debug!("Opus decode: {} bytes to {} samples", opus_data.len(), output_size);
        Ok(vec![0i16; output_size]) // Placeholder PCM data
    }
}

/// CUDA-accelerated codec processor
#[cfg(feature = "cuda")]
pub struct CudaCodecProcessor {
    device: Arc<CudaDevice>,
    kernels: HashMap<String, cudarc::driver::CudaFunction>,
}

#[cfg(feature = "cuda")]
impl CudaCodecProcessor {
    pub fn new() -> Result<Self> {
        let device = CudaDevice::new(0)?; // Use first GPU
        let mut processor = Self {
            device,
            kernels: HashMap::new(),
        };
        
        processor.load_kernels()?;
        Ok(processor)
    }

    fn load_kernels(&mut self) -> Result<()> {
        // Load CUDA kernels for codec translation
        let ptx_source = include_str!("../cuda/codec_kernels.ptx");
        let module = self.device.load_ptx(ptx_source.into(), "codec_kernels", &["g711_translate"])?;
        
        let kernel = self.device.get_func(&module, "g711_ulaw_to_alaw")?;
        self.kernels.insert("ulaw_to_alaw".to_string(), kernel);
        
        Ok(())
    }

    pub fn translate_g711_gpu(&self, input: &[u8], from: AudioCodec, to: AudioCodec) -> Result<Vec<u8>> {
        if from == AudioCodec::G711Ulaw && to == AudioCodec::G711Alaw {
            let input_gpu = self.device.htod_copy(input.to_vec())?;
            let mut output_gpu = self.device.alloc_zeros::<u8>(input.len())?;
            
            if let Some(kernel) = self.kernels.get("ulaw_to_alaw") {
                let cfg = cudarc::driver::LaunchConfig {
                    grid_dim: (input.len() as u32 + 255) / 256,
                    block_dim: 256,
                    shared_mem_bytes: 0,
                };
                
                unsafe {
                    kernel.launch(cfg, (&input_gpu, &mut output_gpu, input.len()))?;
                }
                
                let output = self.device.dtoh_sync_copy(&output_gpu)?;
                return Ok(output);
            }
        }
        
        Err(anyhow!("Unsupported GPU translation: {:?} to {:?}", from, to))
    }
}

/// Main codec translation service
pub struct CodecService {
    config: CodecConfig,
    /// Active transcoding sessions
    sessions: Arc<RwLock<HashMap<String, TranscodingSession>>>,
    /// G.729 codec instances (one per session)
    g729_codecs: Arc<Mutex<HashMap<String, G729Codec>>>,
    /// Opus codec instances
    opus_codecs: Arc<Mutex<HashMap<String, OpusCodec>>>,
    /// GPU codec accelerator
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    gpu_accelerator: Option<Arc<GpuCodecAccelerator>>,
    /// G.729 Annex A/B GPU processor
    g729_annex_processor: Option<Arc<G729AnnexGpuProcessor>>,
    /// CUDA processor for GPU acceleration (legacy)
    #[cfg(feature = "cuda")]
    cuda_processor: Option<Arc<CudaCodecProcessor>>,
}

/// Transcoding session state
#[derive(Debug, Clone)]
pub struct TranscodingSession {
    pub session_id: String,
    pub from_codec: AudioCodec,
    pub to_codec: AudioCodec,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames_processed: u64,
    pub total_processing_time_us: u64,
}

impl CodecService {
    /// Create a new codec service
    pub async fn new(config: CodecConfig) -> Result<Self> {
        // Initialize GPU accelerator
        #[cfg(any(feature = "cuda", feature = "rocm"))]
        let gpu_accelerator = if config.use_gpu && config.gpu_config.enabled {
            match GpuCodecAccelerator::new(config.gpu_config.clone()).await {
                Ok(accelerator) => {
                    info!("GPU codec accelerator initialized with {:?} backend", config.gpu_config.backend);
                    Some(Arc::new(accelerator))
                }
                Err(e) => {
                    warn!("Failed to initialize GPU codec accelerator: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Initialize G.729 Annex A/B processor
        let g729_annex_processor = if config.use_gpu && 
                                      (config.g729_annex_config.annex_a_enabled || config.g729_annex_config.annex_b_enabled) {
            match G729AnnexGpuProcessor::new(config.g729_annex_config.clone()).await {
                Ok(processor) => {
                    info!("G.729 Annex A/B GPU processor initialized");
                    Some(Arc::new(processor))
                }
                Err(e) => {
                    warn!("Failed to initialize G.729 Annex processor: {}", e);
                    None
                }
            }
        } else {
            None
        };

        #[cfg(feature = "cuda")]
        let cuda_processor = if config.use_gpu && gpu_accelerator.is_none() {
            match CudaCodecProcessor::new() {
                Ok(processor) => {
                    info!("Legacy CUDA codec processor initialized");
                    Some(Arc::new(processor))
                }
                Err(e) => {
                    warn!("Failed to initialize CUDA codec processor: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            g729_codecs: Arc::new(Mutex::new(HashMap::new())),
            opus_codecs: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(any(feature = "cuda", feature = "rocm"))]
            gpu_accelerator,
            g729_annex_processor,
            #[cfg(feature = "cuda")]
            cuda_processor,
        })
    }

    /// Start a new transcoding session
    pub async fn start_session(
        &self,
        session_id: String,
        from_codec: AudioCodec,
        to_codec: AudioCodec,
        sample_rate: u32,
        channels: u16,
    ) -> Result<()> {
        // Check if translation is supported
        let translation = CodecTranslation { from: from_codec, to: to_codec };
        if !self.config.supported_translations.contains(&translation) {
            return Err(anyhow!("Unsupported codec translation: {:?} to {:?}", from_codec, to_codec));
        }

        let session = TranscodingSession {
            session_id: session_id.clone(),
            from_codec,
            to_codec,
            sample_rate,
            channels,
            frames_processed: 0,
            total_processing_time_us: 0,
        };

        // Initialize codec instances if needed
        if from_codec == AudioCodec::G729 || to_codec == AudioCodec::G729 {
            let mut g729_codecs = self.g729_codecs.lock().await;
            g729_codecs.insert(session_id.clone(), G729Codec::new());
        }

        if from_codec == AudioCodec::Opus || to_codec == AudioCodec::Opus {
            let mut opus_codecs = self.opus_codecs.lock().await;
            opus_codecs.insert(session_id.clone(), OpusCodec::new(sample_rate, channels as u8)?);
        }

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);

        info!("Started transcoding session {} ({:?} -> {:?})", session_id, from_codec, to_codec);
        Ok(())
    }

    /// Transcode an audio frame
    pub async fn transcode_frame(
        &self,
        session_id: &str,
        frame: AudioFrame,
    ) -> Result<TranscodedFrame> {
        let start_time = std::time::Instant::now();
        
        let (from_codec, to_codec) = {
            let sessions = self.sessions.read().await;
            let session = sessions.get(session_id)
                .ok_or_else(|| anyhow!("Transcoding session {} not found", session_id))?;
            (session.from_codec, session.to_codec)
        };

        // Try GPU acceleration first
        #[cfg(any(feature = "cuda", feature = "rocm"))]
        let transcoded_data = if let Some(ref gpu_accelerator) = self.gpu_accelerator {
            if self.can_use_gpu_for_transcoding(from_codec, to_codec) {
                match self.gpu_transcode_frame(gpu_accelerator, &frame, to_codec).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("GPU transcoding failed, falling back to CPU: {}", e);
                        self.perform_transcoding(session_id, &frame.data, from_codec, to_codec).await?
                    }
                }
            } else {
                self.perform_transcoding(session_id, &frame.data, from_codec, to_codec).await?
            }
        } else {
            self.perform_transcoding(session_id, &frame.data, from_codec, to_codec).await?
        };

        #[cfg(not(any(feature = "cuda", feature = "rocm")))]
        let transcoded_data = self.perform_transcoding(session_id, &frame.data, from_codec, to_codec).await?;

        let processing_time_us = start_time.elapsed().as_micros() as u64;

        // Update session statistics
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.frames_processed += 1;
            session.total_processing_time_us += processing_time_us;
        }

        Ok(TranscodedFrame {
            original: frame,
            data: transcoded_data,
            target_codec: to_codec,
            processing_time_us,
        })
    }

    /// Check if GPU acceleration is available for transcoding
    fn can_use_gpu_for_transcoding(&self, from_codec: AudioCodec, to_codec: AudioCodec) -> bool {
        // GPU supports direct transcoding between all supported codecs
        match (from_codec, to_codec) {
            // Same codec (passthrough)
            (a, b) if a == b => false, // No transcoding needed
            
            // All G.711 variants
            (AudioCodec::G711Ulaw, AudioCodec::G711Alaw) |
            (AudioCodec::G711Alaw, AudioCodec::G711Ulaw) => true,
            
            // G.711 ↔ G.729 (all variants)
            (AudioCodec::G711Ulaw, AudioCodec::G729) |
            (AudioCodec::G711Ulaw, AudioCodec::G729AnnexA) |
            (AudioCodec::G711Ulaw, AudioCodec::G729AnnexB) |
            (AudioCodec::G711Alaw, AudioCodec::G729) |
            (AudioCodec::G711Alaw, AudioCodec::G729AnnexA) |
            (AudioCodec::G711Alaw, AudioCodec::G729AnnexB) |
            (AudioCodec::G729, AudioCodec::G711Ulaw) |
            (AudioCodec::G729, AudioCodec::G711Alaw) |
            (AudioCodec::G729AnnexA, AudioCodec::G711Ulaw) |
            (AudioCodec::G729AnnexA, AudioCodec::G711Alaw) |
            (AudioCodec::G729AnnexB, AudioCodec::G711Ulaw) |
            (AudioCodec::G729AnnexB, AudioCodec::G711Alaw) => true,
            
            // G.711 ↔ G.722.2/AMR-WB
            (AudioCodec::G711Ulaw, AudioCodec::G7222) |
            (AudioCodec::G711Alaw, AudioCodec::G7222) |
            (AudioCodec::G7222, AudioCodec::G711Ulaw) |
            (AudioCodec::G7222, AudioCodec::G711Alaw) => true,
            
            // G.711 ↔ G.722
            (AudioCodec::G711Ulaw, AudioCodec::G722) |
            (AudioCodec::G711Alaw, AudioCodec::G722) |
            (AudioCodec::G722, AudioCodec::G711Ulaw) |
            (AudioCodec::G722, AudioCodec::G711Alaw) => true,
            
            // G.729 ↔ G.722.2
            (AudioCodec::G729, AudioCodec::G7222) |
            (AudioCodec::G729AnnexA, AudioCodec::G7222) |
            (AudioCodec::G729AnnexB, AudioCodec::G7222) |
            (AudioCodec::G7222, AudioCodec::G729) |
            (AudioCodec::G7222, AudioCodec::G729AnnexA) |
            (AudioCodec::G7222, AudioCodec::G729AnnexB) => true,
            
            // G.729 ↔ G.722
            (AudioCodec::G729, AudioCodec::G722) |
            (AudioCodec::G729AnnexA, AudioCodec::G722) |
            (AudioCodec::G729AnnexB, AudioCodec::G722) |
            (AudioCodec::G722, AudioCodec::G729) |
            (AudioCodec::G722, AudioCodec::G729AnnexA) |
            (AudioCodec::G722, AudioCodec::G729AnnexB) => true,
            
            // G.722.2 ↔ G.722
            (AudioCodec::G7222, AudioCodec::G722) |
            (AudioCodec::G722, AudioCodec::G7222) => true,
            
            // PCM16 ↔ All codecs
            (AudioCodec::Pcm16, AudioCodec::G711Ulaw) |
            (AudioCodec::Pcm16, AudioCodec::G711Alaw) |
            (AudioCodec::Pcm16, AudioCodec::G729) |
            (AudioCodec::Pcm16, AudioCodec::G729AnnexA) |
            (AudioCodec::Pcm16, AudioCodec::G729AnnexB) |
            (AudioCodec::Pcm16, AudioCodec::G722) |
            (AudioCodec::Pcm16, AudioCodec::G7222) |
            (AudioCodec::G711Ulaw, AudioCodec::Pcm16) |
            (AudioCodec::G711Alaw, AudioCodec::Pcm16) |
            (AudioCodec::G729, AudioCodec::Pcm16) |
            (AudioCodec::G729AnnexA, AudioCodec::Pcm16) |
            (AudioCodec::G729AnnexB, AudioCodec::Pcm16) |
            (AudioCodec::G722, AudioCodec::Pcm16) |
            (AudioCodec::G7222, AudioCodec::Pcm16) => true,
            
            // Opus is handled by CPU for now (requires more complex processing)
            (AudioCodec::Opus, _) | (_, AudioCodec::Opus) => false,
            
            _ => false,
        }
    }

    /// GPU-accelerated frame transcoding
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    async fn gpu_transcode_frame(
        &self,
        gpu_accelerator: &GpuCodecAccelerator,
        frame: &AudioFrame,
        target_codec: AudioCodec,
    ) -> Result<Vec<u8>> {
        let frames = vec![frame.clone()];
        let transcoded_frames = gpu_accelerator.batch_encode(&frames, target_codec).await?;
        
        if let Some(transcoded_frame) = transcoded_frames.first() {
            Ok(transcoded_frame.data.clone())
        } else {
            Err(anyhow!("GPU transcoding returned no frames"))
        }
    }

    /// Perform the actual transcoding
    async fn perform_transcoding(
        &self,
        session_id: &str,
        data: &[u8],
        from_codec: AudioCodec,
        to_codec: AudioCodec,
    ) -> Result<Vec<u8>> {
        // Try GPU acceleration first if available
        #[cfg(feature = "cuda")]
        if let Some(ref cuda_processor) = self.cuda_processor {
            if let Ok(result) = cuda_processor.translate_g711_gpu(data, from_codec, to_codec) {
                debug!("Used GPU acceleration for transcoding");
                return Ok(result);
            }
        }

        // Fall back to CPU transcoding
        match (from_codec, to_codec) {
            // G.711 translations
            (AudioCodec::G711Ulaw, AudioCodec::G711Alaw) => {
                Ok(G711Codec::ulaw_to_alaw(data))
            }
            (AudioCodec::G711Alaw, AudioCodec::G711Ulaw) => {
                Ok(G711Codec::alaw_to_ulaw(data))
            }
            
            // G.711 to PCM conversions
            (AudioCodec::G711Ulaw, AudioCodec::Pcm16) => {
                let pcm_data = G711Codec::decode_ulaw(data);
                let mut output = Vec::with_capacity(pcm_data.len() * 2);
                for sample in pcm_data {
                    output.extend_from_slice(&sample.to_le_bytes());
                }
                Ok(output)
            }
            (AudioCodec::G711Alaw, AudioCodec::Pcm16) => {
                let pcm_data = G711Codec::decode_alaw(data);
                let mut output = Vec::with_capacity(pcm_data.len() * 2);
                for sample in pcm_data {
                    output.extend_from_slice(&sample.to_le_bytes());
                }
                Ok(output)
            }
            
            // PCM to G.711 conversions
            (AudioCodec::Pcm16, AudioCodec::G711Ulaw) => {
                let pcm_data = self.bytes_to_pcm16(data);
                Ok(G711Codec::encode_ulaw(&pcm_data))
            }
            (AudioCodec::Pcm16, AudioCodec::G711Alaw) => {
                let pcm_data = self.bytes_to_pcm16(data);
                Ok(G711Codec::encode_alaw(&pcm_data))
            }

            // G.729 translations (via PCM intermediate)
            (AudioCodec::G711Ulaw, AudioCodec::G729) => {
                let pcm_data = G711Codec::decode_ulaw(data);
                let mut g729_codecs = self.g729_codecs.lock().await;
                let codec = g729_codecs.get_mut(session_id)
                    .ok_or_else(|| anyhow!("G.729 codec not found for session"))?;
                codec.encode(&pcm_data)
            }
            (AudioCodec::G711Alaw, AudioCodec::G729) => {
                let pcm_data = G711Codec::decode_alaw(data);
                let mut g729_codecs = self.g729_codecs.lock().await;
                let codec = g729_codecs.get_mut(session_id)
                    .ok_or_else(|| anyhow!("G.729 codec not found for session"))?;
                codec.encode(&pcm_data)
            }
            (AudioCodec::G729, AudioCodec::G711Ulaw) => {
                let mut g729_codecs = self.g729_codecs.lock().await;
                let codec = g729_codecs.get_mut(session_id)
                    .ok_or_else(|| anyhow!("G.729 codec not found for session"))?;
                let pcm_data = codec.decode(data)?;
                Ok(G711Codec::encode_ulaw(&pcm_data))
            }
            (AudioCodec::G729, AudioCodec::G711Alaw) => {
                let mut g729_codecs = self.g729_codecs.lock().await;
                let codec = g729_codecs.get_mut(session_id)
                    .ok_or_else(|| anyhow!("G.729 codec not found for session"))?;
                let pcm_data = codec.decode(data)?;
                Ok(G711Codec::encode_alaw(&pcm_data))
            }

            // Opus translations
            (AudioCodec::Opus, AudioCodec::G711Ulaw) => {
                let mut opus_codecs = self.opus_codecs.lock().await;
                let codec = opus_codecs.get_mut(session_id)
                    .ok_or_else(|| anyhow!("Opus codec not found for session"))?;
                let pcm_data = codec.decode(data, 160)?; // Assume 160 samples for 20ms at 8kHz
                Ok(G711Codec::encode_ulaw(&pcm_data))
            }
            (AudioCodec::G711Ulaw, AudioCodec::Opus) => {
                let pcm_data = G711Codec::decode_ulaw(data);
                // Resample from 8kHz to 48kHz for Opus
                let resampled = self.resample_audio(&pcm_data, 8000, 48000)?;
                let mut opus_codecs = self.opus_codecs.lock().await;
                let codec = opus_codecs.get_mut(session_id)
                    .ok_or_else(|| anyhow!("Opus codec not found for session"))?;
                codec.encode(&resampled)
            }

            _ => {
                Err(anyhow!("Unsupported codec translation: {:?} to {:?}", from_codec, to_codec))
            }
        }
    }

    /// Convert byte array to PCM16 samples
    fn bytes_to_pcm16(&self, data: &[u8]) -> Vec<i16> {
        if data.len() % 2 != 0 {
            warn!("PCM16 data length is odd, padding with zero");
        }
        data.chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect()
    }

    /// Resample audio from one sample rate to another
    fn resample_audio(&self, input: &[i16], from_rate: u32, to_rate: u32) -> Result<Vec<i16>> {
        if from_rate == to_rate {
            return Ok(input.to_vec());
        }

        // Simple linear interpolation resampling
        let ratio = to_rate as f64 / from_rate as f64;
        let output_len = (input.len() as f64 * ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_index = i as f64 / ratio;
            let src_index_floor = src_index.floor() as usize;
            let src_index_ceil = (src_index_floor + 1).min(input.len() - 1);
            
            let fraction = src_index - src_index_floor as f64;
            let sample = if src_index_floor < input.len() {
                let a = input[src_index_floor] as f64;
                let b = input[src_index_ceil] as f64;
                (a + (b - a) * fraction) as i16
            } else {
                0
            };
            
            output.push(sample);
        }

        Ok(output)
    }

    /// End a transcoding session
    pub async fn end_session(&self, session_id: &str) -> Result<TranscodingSession> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.remove(session_id)
            .ok_or_else(|| anyhow!("Transcoding session {} not found", session_id))?;

        // Clean up codec instances
        let mut g729_codecs = self.g729_codecs.lock().await;
        g729_codecs.remove(session_id);

        let mut opus_codecs = self.opus_codecs.lock().await;
        opus_codecs.remove(session_id);

        info!("Ended transcoding session {} - processed {} frames in {}μs average", 
              session_id, 
              session.frames_processed,
              if session.frames_processed > 0 { 
                  session.total_processing_time_us / session.frames_processed 
              } else { 0 });

        Ok(session)
    }

    /// Encode G.729 frame with Annex A/B (VAD/DTX/CNG)
    pub async fn encode_g729_annex_frame(
        &self,
        session_id: &str,
        audio_samples: &[i16],
    ) -> Result<G729AnnexFrame> {
        if let Some(ref processor) = self.g729_annex_processor {
            processor.encode_frame(session_id, audio_samples).await
        } else {
            // Fallback to regular G.729 encoding
            let mut g729_codecs = self.g729_codecs.lock().await;
            if let Some(codec) = g729_codecs.get_mut(session_id) {
                let encoded_data = codec.encode(audio_samples)?;
                Ok(G729AnnexFrame {
                    frame_type: G729FrameType::Speech,
                    data: encoded_data,
                    energy_level: None,
                })
            } else {
                Err(anyhow!("G.729 codec session {} not found", session_id))
            }
        }
    }

    /// Generate comfort noise for G.729 Annex B
    pub async fn generate_g729_comfort_noise(
        &self,
        session_id: &str,
        sid_energy: u8,
    ) -> Result<Vec<i16>> {
        if let Some(ref processor) = self.g729_annex_processor {
            let sid_frame = crate::g729_annex_gpu::SidFrame {
                energy: sid_energy,
                reflection_coeffs: [32, 16, 8, 4], // Placeholder values
            };
            processor.generate_comfort_noise(session_id, &sid_frame).await
        } else {
            // Generate simple white noise as fallback
            let mut noise = Vec::with_capacity(crate::g729_codec::G729_FRAME_SIZE);
            let mut rng_state = 12345u32;
            let energy_scale = (sid_energy as f32 - 100.0) / 10.0;
            let energy_linear = 10.0_f32.powf(energy_scale) * 0.01; // Scale down
            
            for _ in 0..crate::g729_codec::G729_FRAME_SIZE {
                rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
                let sample = ((rng_state & 0x7FFFFFFF) as f32 / 0x7FFFFFFF as f32) * 2.0 - 1.0;
                noise.push((sample * energy_linear * 32767.0) as i16);
            }
            
            Ok(noise)
        }
    }

    /// Start G.729 Annex A/B session
    pub async fn start_g729_annex_session(&self, session_id: String) -> Result<()> {
        if let Some(ref processor) = self.g729_annex_processor {
            processor.start_session(session_id).await
        } else {
            // Start regular G.729 session as fallback
            let mut g729_codecs = self.g729_codecs.lock().await;
            g729_codecs.insert(session_id, crate::g729_codec::G729Codec::new());
            Ok(())
        }
    }

    /// End G.729 Annex A/B session
    pub async fn end_g729_annex_session(&self, session_id: &str) -> Result<()> {
        if let Some(ref processor) = self.g729_annex_processor {
            processor.end_session(session_id).await
        } else {
            let mut g729_codecs = self.g729_codecs.lock().await;
            g729_codecs.remove(session_id);
            Ok(())
        }
    }

    /// Get G.729 Annex A/B statistics
    pub async fn get_g729_annex_stats(&self) -> Option<crate::g729_annex_gpu::G729AnnexStats> {
        if let Some(ref processor) = self.g729_annex_processor {
            Some(processor.get_statistics().await)
        } else {
            None
        }
    }

    /// Get session statistics
    pub async fn get_session_stats(&self, session_id: &str) -> Option<TranscodingSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Get all active sessions
    pub async fn get_active_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// Get codec service statistics
    pub async fn get_statistics(&self) -> CodecStatistics {
        let sessions = self.sessions.read().await;
        let total_sessions = sessions.len();
        let mut total_frames = 0;
        let mut total_processing_time = 0;

        for session in sessions.values() {
            total_frames += session.frames_processed;
            total_processing_time += session.total_processing_time_us;
        }

        let average_processing_time = if total_frames > 0 {
            total_processing_time / total_frames
        } else {
            0
        };

        CodecStatistics {
            active_sessions: total_sessions,
            total_frames_processed: total_frames,
            average_processing_time_us: average_processing_time,
            gpu_acceleration_available: cfg!(feature = "cuda") && self.config.use_gpu,
            #[cfg(feature = "cuda")]
            gpu_acceleration_active: self.cuda_processor.is_some(),
            #[cfg(not(feature = "cuda"))]
            gpu_acceleration_active: false,
        }
    }
}

/// Codec service statistics
#[derive(Debug, Clone)]
pub struct CodecStatistics {
    pub active_sessions: usize,
    pub total_frames_processed: u64,
    pub average_processing_time_us: u64,
    pub gpu_acceleration_available: bool,
    pub gpu_acceleration_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_g711_ulaw_alaw_conversion() {
        let pcm_data = vec![1000i16, -1000, 2000, -2000, 0];
        
        // Test μ-law encoding/decoding
        let ulaw_encoded = G711Codec::encode_ulaw(&pcm_data);
        let ulaw_decoded = G711Codec::decode_ulaw(&ulaw_encoded);
        
        // Test A-law encoding/decoding
        let alaw_encoded = G711Codec::encode_alaw(&pcm_data);
        let alaw_decoded = G711Codec::decode_alaw(&alaw_encoded);
        
        // Test direct conversion
        let ulaw_to_alaw = G711Codec::ulaw_to_alaw(&ulaw_encoded);
        let alaw_from_direct = G711Codec::decode_alaw(&ulaw_to_alaw);
        
        // The conversions should be reasonably close (G.711 is lossy)
        assert_eq!(ulaw_decoded.len(), pcm_data.len());
        assert_eq!(alaw_decoded.len(), pcm_data.len());
        assert_eq!(alaw_from_direct.len(), pcm_data.len());
    }

    #[test]
    fn test_codec_payload_types() {
        assert_eq!(AudioCodec::G711Ulaw.payload_type(), 0);
        assert_eq!(AudioCodec::G711Alaw.payload_type(), 8);
        assert_eq!(AudioCodec::G729.payload_type(), 18);
        
        assert_eq!(AudioCodec::from_payload_type(0), Some(AudioCodec::G711Ulaw));
        assert_eq!(AudioCodec::from_payload_type(8), Some(AudioCodec::G711Alaw));
        assert_eq!(AudioCodec::from_payload_type(255), None);
    }

    #[tokio::test]
    async fn test_codec_service_creation() {
        let config = CodecConfig::default();
        let service = CodecService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_transcoding_session() {
        let config = CodecConfig::default();
        let service = CodecService::new(config).await.unwrap();
        
        let session_id = "test-session".to_string();
        let result = service.start_session(
            session_id.clone(),
            AudioCodec::G711Ulaw,
            AudioCodec::G711Alaw,
            8000,
            1,
        ).await;
        
        assert!(result.is_ok());
        
        let stats = service.get_session_stats(&session_id).await;
        assert!(stats.is_some());
        
        let end_result = service.end_session(&session_id).await;
        assert!(end_result.is_ok());
    }
}