/*
 * Universal GPU Transcoding Service
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * Provides direct GPU transcoding between all supported codec pairs
 */

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "cuda")]
use cudarc::driver::{CudaDevice, DevicePtr, LaunchAsync, LaunchConfig};

use crate::codec::{AudioCodec, AudioFrame, TranscodedFrame};

/// Codec type mapping for GPU kernels
const CODEC_G711_ULAW: u8 = 0;
const CODEC_G711_ALAW: u8 = 1;
const CODEC_G729: u8 = 2;
const CODEC_G729_ANNEX_A: u8 = 3;
const CODEC_G729_ANNEX_B: u8 = 4;
const CODEC_PCM16: u8 = 5;
const CODEC_G722: u8 = 6;
const CODEC_G7222: u8 = 7;
const CODEC_OPUS: u8 = 8;

/// Universal GPU transcoding accelerator
#[cfg(feature = "cuda")]
pub struct UniversalGpuTranscoder {
    device: Arc<CudaDevice>,
    kernels: HashMap<String, cudarc::driver::CudaFunction>,
    device_memory_pool: DeviceMemoryPool,
    stream: cudarc::driver::CudaStream,
}

#[cfg(feature = "cuda")]
struct DeviceMemoryPool {
    input_buffer: DevicePtr<u8>,
    output_buffer: DevicePtr<u8>,
    state_buffer: DevicePtr<UniversalCodecState>,
    input_sizes: DevicePtr<i32>,
    output_sizes: DevicePtr<i32>,
    src_codecs: DevicePtr<u8>,
    dst_codecs: DevicePtr<u8>,
    input_offsets: DevicePtr<i32>,
    output_offsets: DevicePtr<i32>,
    max_batch_size: usize,
}

/// GPU codec state structure (matches CUDA struct)
#[repr(C)]
#[derive(Clone, Copy)]
struct UniversalCodecState {
    // G.729 state
    g729_old_exc: [f32; 240],
    g729_old_lsp: [f32; 10],
    g729_mem_syn: [f32; 10],
    g729_mem_deemph: f32,
    
    // G.722.2 state  
    g7222_old_exc: [f32; 640],
    g7222_old_isp: [f32; 16],
    g7222_mem_syn: [f32; 16],
    g7222_mem_deemph: f32,
    
    // G.722 state
    g722_x: [f32; 24],
    g722_h: [f32; 24],
    g722_s1: i32,
    g722_s2: i32,
    
    // General resampling state
    resample_history: [f32; 32],
    
    // Gain control
    auto_gain: f32,
}

impl Default for UniversalCodecState {
    fn default() -> Self {
        Self {
            g729_old_exc: [0.0; 240],
            g729_old_lsp: [0.0; 10],
            g729_mem_syn: [0.0; 10],
            g729_mem_deemph: 0.0,
            g7222_old_exc: [0.0; 640],
            g7222_old_isp: [0.0; 16],
            g7222_mem_syn: [0.0; 16],
            g7222_mem_deemph: 0.0,
            g722_x: [0.0; 24],
            g722_h: [0.0; 24],
            g722_s1: 0,
            g722_s2: 0,
            resample_history: [0.0; 32],
            auto_gain: 1.0,
        }
    }
}

#[cfg(feature = "cuda")]
impl UniversalGpuTranscoder {
    /// Create new GPU transcoder with maximum batch size
    pub fn new(max_batch_size: usize) -> Result<Self> {
        let device = Arc::new(CudaDevice::new(0)?);
        let stream = device.fork_default_stream()?;
        
        // Allocate device memory pool
        let memory_pool = DeviceMemoryPool::new(&device, max_batch_size)?;
        
        let mut transcoder = Self {
            device,
            kernels: HashMap::new(),
            device_memory_pool: memory_pool,
            stream,
        };
        
        transcoder.load_kernels()?;
        Ok(transcoder)
    }
    
    /// Load and compile GPU kernels
    fn load_kernels(&mut self) -> Result<()> {
        // Load pre-compiled PTX or compile from source
        let ptx_source = include_str!("universal_codec_transcode.cu");
        
        // Compile kernels
        let ptx = cudarc::nvrtc::compile_ptx(ptx_source)?;
        self.device.load_ptx(ptx, "universal_transcode", &[
            "universal_transcode_kernel",
            "g711_ulaw_alaw_direct_kernel", 
            "g711_alaw_ulaw_direct_kernel",
            "batch_universal_transcode_kernel"
        ])?;
        
        // Get kernel functions
        let universal_kernel = self.device.get_func("universal_transcode", "universal_transcode_kernel")?;
        let ulaw_alaw_kernel = self.device.get_func("universal_transcode", "g711_ulaw_alaw_direct_kernel")?;
        let alaw_ulaw_kernel = self.device.get_func("universal_transcode", "g711_alaw_ulaw_direct_kernel")?;
        let batch_kernel = self.device.get_func("universal_transcode", "batch_universal_transcode_kernel")?;
        
        self.kernels.insert("universal".to_string(), universal_kernel);
        self.kernels.insert("ulaw_to_alaw".to_string(), ulaw_alaw_kernel);
        self.kernels.insert("alaw_to_ulaw".to_string(), alaw_ulaw_kernel);
        self.kernels.insert("batch".to_string(), batch_kernel);
        
        Ok(())
    }
    
    /// Transcode a single frame
    pub async fn transcode_frame(
        &self,
        input_frame: &AudioFrame,
        target_codec: AudioCodec,
    ) -> Result<TranscodedFrame> {
        let frames = vec![input_frame.clone()];
        let mut results = self.batch_transcode(&frames, &[target_codec]).await?;
        
        if let Some(result) = results.pop() {
            Ok(result)
        } else {
            Err(anyhow!("GPU transcoding returned no results"))
        }
    }
    
    /// Batch transcode multiple frames
    pub async fn batch_transcode(
        &self,
        input_frames: &[AudioFrame],
        target_codecs: &[AudioCodec],
    ) -> Result<Vec<TranscodedFrame>> {
        if input_frames.len() != target_codecs.len() {
            return Err(anyhow!("Input frames and target codecs length mismatch"));
        }
        
        if input_frames.len() > self.device_memory_pool.max_batch_size {
            return Err(anyhow!("Batch size exceeds maximum"));
        }
        
        let start_time = std::time::Instant::now();
        
        // Check for optimized direct conversions
        if input_frames.len() == 1 {
            let input = &input_frames[0];
            let target = target_codecs[0];
            
            if let Some(result) = self.try_direct_conversion(input, target).await? {
                return Ok(vec![result]);
            }
        }
        
        // Prepare batch data
        let (input_data, input_sizes, input_offsets) = self.prepare_input_batch(input_frames)?;
        let (output_sizes, output_offsets) = self.calculate_output_batch(input_frames, target_codecs)?;
        let src_codecs = self.map_codecs_to_gpu_types(input_frames.iter().map(|f| f.codec))?;
        let dst_codecs = self.map_codecs_to_gpu_types(target_codecs.iter().copied())?;
        
        // Copy data to GPU
        self.device.dtoh_sync_copy_into(&input_data, &self.device_memory_pool.input_buffer)?;
        self.device.dtoh_sync_copy_into(&input_sizes, &self.device_memory_pool.input_sizes)?;
        self.device.dtoh_sync_copy_into(&output_sizes, &self.device_memory_pool.output_sizes)?;
        self.device.dtoh_sync_copy_into(&src_codecs, &self.device_memory_pool.src_codecs)?;
        self.device.dtoh_sync_copy_into(&dst_codecs, &self.device_memory_pool.dst_codecs)?;
        self.device.dtoh_sync_copy_into(&input_offsets, &self.device_memory_pool.input_offsets)?;
        self.device.dtoh_sync_copy_into(&output_offsets, &self.device_memory_pool.output_offsets)?;
        
        // Launch kernel
        let kernel = self.kernels.get("batch").ok_or_else(|| anyhow!("Batch kernel not found"))?;
        
        let grid_size = (input_frames.len() as u32 + 255) / 256;
        let block_size = 256;
        
        let config = LaunchConfig {
            grid_dim: (grid_size, 1, 1),
            block_dim: (block_size, 1, 1),
            shared_mem_bytes: 0,
        };
        
        unsafe {
            kernel.launch_async(config, (
                &self.device_memory_pool.input_buffer,
                &self.device_memory_pool.output_buffer,
                &self.device_memory_pool.state_buffer,
                &self.device_memory_pool.input_offsets,
                &self.device_memory_pool.output_offsets,
                &self.device_memory_pool.src_codecs,
                &self.device_memory_pool.dst_codecs,
                input_frames.len() as i32,
            ), &self.stream)?;
        }
        
        // Wait for completion and copy results back
        self.stream.synchronize()?;
        
        let total_output_size: usize = output_sizes.iter().sum::<i32>() as usize;
        let mut output_data = vec![0u8; total_output_size];
        self.device.synchronize()?;
        self.device.dtoh_sync_copy_into(&self.device_memory_pool.output_buffer, &mut output_data)?;
        
        // Build result frames
        let processing_time_us = start_time.elapsed().as_micros() as u64;
        let mut results = Vec::new();
        let mut offset = 0;
        
        for (i, input_frame) in input_frames.iter().enumerate() {
            let size = output_sizes[i] as usize;
            let transcoded_data = output_data[offset..offset + size].to_vec();
            
            results.push(TranscodedFrame {
                original: input_frame.clone(),
                data: transcoded_data,
                target_codec: target_codecs[i],
                processing_time_us: processing_time_us / input_frames.len() as u64,
            });
            
            offset += size;
        }
        
        Ok(results)
    }
    
    /// Try optimized direct conversion for common pairs
    async fn try_direct_conversion(
        &self,
        input: &AudioFrame,
        target: AudioCodec,
    ) -> Result<Option<TranscodedFrame>> {
        let start_time = std::time::Instant::now();
        
        match (input.codec, target) {
            (AudioCodec::G711Ulaw, AudioCodec::G711Alaw) => {
                let kernel = self.kernels.get("ulaw_to_alaw").ok_or_else(|| anyhow!("Kernel not found"))?;
                let result = self.run_direct_kernel(kernel, &input.data, input.data.len()).await?;
                
                Ok(Some(TranscodedFrame {
                    original: input.clone(),
                    data: result,
                    target_codec: target,
                    processing_time_us: start_time.elapsed().as_micros() as u64,
                }))
            }
            
            (AudioCodec::G711Alaw, AudioCodec::G711Ulaw) => {
                let kernel = self.kernels.get("alaw_to_ulaw").ok_or_else(|| anyhow!("Kernel not found"))?;
                let result = self.run_direct_kernel(kernel, &input.data, input.data.len()).await?;
                
                Ok(Some(TranscodedFrame {
                    original: input.clone(),
                    data: result,
                    target_codec: target,
                    processing_time_us: start_time.elapsed().as_micros() as u64,
                }))
            }
            
            _ => Ok(None), // Use universal kernel
        }
    }
    
    /// Run optimized direct conversion kernel
    async fn run_direct_kernel(
        &self,
        kernel: &cudarc::driver::CudaFunction,
        input_data: &[u8],
        output_size: usize,
    ) -> Result<Vec<u8>> {
        // Allocate temporary buffers
        let input_gpu = self.device.htod_copy(input_data.to_vec())?;
        let output_gpu = self.device.alloc_zeros::<u8>(output_size)?;
        
        let grid_size = (input_data.len() as u32 + 255) / 256;
        let config = LaunchConfig {
            grid_dim: (grid_size, 1, 1),
            block_dim: (256, 1, 1),
            shared_mem_bytes: 0,
        };
        
        unsafe {
            kernel.launch_async(config, (
                &input_gpu,
                &output_gpu,
                input_data.len() as i32,
            ), &self.stream)?;
        }
        
        self.stream.synchronize()?;
        let result = self.device.dtoh_sync_copy(&output_gpu)?;
        
        Ok(result)
    }
    
    /// Prepare input batch data
    fn prepare_input_batch(&self, frames: &[AudioFrame]) -> Result<(Vec<u8>, Vec<i32>, Vec<i32>)> {
        let mut input_data = Vec::new();
        let mut input_sizes = Vec::new();
        let mut input_offsets = Vec::new();
        
        for frame in frames {
            input_offsets.push(input_data.len() as i32);
            input_data.extend_from_slice(&frame.data);
            input_sizes.push(frame.data.len() as i32);
        }
        
        Ok((input_data, input_sizes, input_offsets))
    }
    
    /// Calculate output batch sizes and offsets
    fn calculate_output_batch(&self, input_frames: &[AudioFrame], target_codecs: &[AudioCodec]) -> Result<(Vec<i32>, Vec<i32>)> {
        let mut output_sizes = Vec::new();
        let mut output_offsets = Vec::new();
        let mut offset = 0;
        
        for (frame, target) in input_frames.iter().zip(target_codecs) {
            output_offsets.push(offset);
            let size = self.estimate_output_size(frame.codec, *target);
            output_sizes.push(size as i32);
            offset += size as i32;
        }
        
        Ok((output_sizes, output_offsets))
    }
    
    /// Estimate output frame size for codec pair
    fn estimate_output_size(&self, src: AudioCodec, dst: AudioCodec) -> usize {
        match dst {
            AudioCodec::G711Ulaw | AudioCodec::G711Alaw => 80,  // 10ms at 8kHz
            AudioCodec::G729 | AudioCodec::G729AnnexA | AudioCodec::G729AnnexB => 10,  // Compressed
            AudioCodec::G722 => 80,  // 4 bits per sample, 160 samples
            AudioCodec::G7222 => 33, // AMR-WB mode 8
            AudioCodec::Pcm16 => {
                match src {
                    AudioCodec::G7222 | AudioCodec::G722 => 640, // 320 samples * 2 bytes
                    _ => 320, // 160 samples * 2 bytes
                }
            }
            AudioCodec::Opus => 120, // Variable, estimate
        }
    }
    
    /// Map AudioCodec to GPU codec type
    fn map_codecs_to_gpu_types<I>(&self, codecs: I) -> Result<Vec<u8>>
    where
        I: Iterator<Item = AudioCodec>,
    {
        codecs.map(|codec| {
            match codec {
                AudioCodec::G711Ulaw => Ok(CODEC_G711_ULAW),
                AudioCodec::G711Alaw => Ok(CODEC_G711_ALAW),
                AudioCodec::G729 => Ok(CODEC_G729),
                AudioCodec::G729AnnexA => Ok(CODEC_G729_ANNEX_A),
                AudioCodec::G729AnnexB => Ok(CODEC_G729_ANNEX_B),
                AudioCodec::Pcm16 => Ok(CODEC_PCM16),
                AudioCodec::G722 => Ok(CODEC_G722),
                AudioCodec::G7222 => Ok(CODEC_G7222),
                AudioCodec::Opus => Ok(CODEC_OPUS),
            }
        }).collect()
    }
    
    /// Get supported transcoding pairs
    pub fn get_supported_pairs() -> Vec<(AudioCodec, AudioCodec)> {
        let mut pairs = Vec::new();
        let codecs = [
            AudioCodec::G711Ulaw,
            AudioCodec::G711Alaw,
            AudioCodec::G729,
            AudioCodec::G729AnnexA,
            AudioCodec::G729AnnexB,
            AudioCodec::G722,
            AudioCodec::G7222,
            AudioCodec::Pcm16,
        ];
        
        // Generate all codec pairs (except same-to-same)
        for &src in &codecs {
            for &dst in &codecs {
                if src != dst {
                    pairs.push((src, dst));
                }
            }
        }
        
        pairs
    }
}

#[cfg(feature = "cuda")]
impl DeviceMemoryPool {
    fn new(device: &CudaDevice, max_batch_size: usize) -> Result<Self> {
        // Allocate worst-case buffers
        let max_input_size = max_batch_size * 960 * 2; // Worst case: Opus PCM
        let max_output_size = max_input_size;
        let max_state_size = max_batch_size;
        
        Ok(Self {
            input_buffer: device.alloc_zeros::<u8>(max_input_size)?,
            output_buffer: device.alloc_zeros::<u8>(max_output_size)?,
            state_buffer: device.alloc_zeros::<UniversalCodecState>(max_state_size)?,
            input_sizes: device.alloc_zeros::<i32>(max_batch_size)?,
            output_sizes: device.alloc_zeros::<i32>(max_batch_size)?,
            src_codecs: device.alloc_zeros::<u8>(max_batch_size)?,
            dst_codecs: device.alloc_zeros::<u8>(max_batch_size)?,
            input_offsets: device.alloc_zeros::<i32>(max_batch_size)?,
            output_offsets: device.alloc_zeros::<i32>(max_batch_size)?,
            max_batch_size,
        })
    }
}

// Placeholder for non-CUDA builds
#[cfg(not(feature = "cuda"))]
pub struct UniversalGpuTranscoder;

#[cfg(not(feature = "cuda"))]
impl UniversalGpuTranscoder {
    pub fn new(_max_batch_size: usize) -> Result<Self> {
        Err(anyhow!("CUDA feature not enabled"))
    }
    
    pub async fn transcode_frame(&self, _input_frame: &AudioFrame, _target_codec: AudioCodec) -> Result<TranscodedFrame> {
        Err(anyhow!("CUDA feature not enabled"))
    }
    
    pub async fn batch_transcode(&self, _input_frames: &[AudioFrame], _target_codecs: &[AudioCodec]) -> Result<Vec<TranscodedFrame>> {
        Err(anyhow!("CUDA feature not enabled"))
    }
    
    pub fn get_supported_pairs() -> Vec<(AudioCodec, AudioCodec)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_supported_pairs_generation() {
        let pairs = UniversalGpuTranscoder::get_supported_pairs();
        assert!(!pairs.is_empty());
        
        // Check that we have G.711 μ-law to A-law
        assert!(pairs.contains(&(AudioCodec::G711Ulaw, AudioCodec::G711Alaw)));
        
        // Check that we have G.729 to G.722.2
        assert!(pairs.contains(&(AudioCodec::G729, AudioCodec::G7222)));
        
        // Check that we don't have same-to-same pairs
        assert!(!pairs.contains(&(AudioCodec::G711Ulaw, AudioCodec::G711Ulaw)));
    }
    
    #[test]
    fn test_codec_state_size() {
        // Ensure the state struct is reasonable size for GPU memory
        let state_size = std::mem::size_of::<UniversalCodecState>();
        assert!(state_size < 8192, "Codec state too large: {} bytes", state_size);
    }
}