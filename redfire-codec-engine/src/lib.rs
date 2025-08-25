/*
 * Redfire Codec Engine - Professional Audio Codec Translation Library
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # Redfire Codec Engine
//!
//! A high-performance audio codec translation engine with GPU acceleration support.
//!
//! ## Features
//!
//! - Multiple audio codec support (G.711, G.729, G.722.2/AMR-WB, Opus, G.722, PCM)
//! - GPU-accelerated transcoding with CUDA and ROCm support
//! - Professional audio resampling with configurable quality
//! - G.729 Annex A/B support with VAD, DTX, and CNG
//! - Real-time performance optimizations
//! - Memory pooling for efficient resource usage
//!
//! ## Basic Usage
//!
//! ```rust
//! use redfire_codec_engine::{CodecService, CodecConfig, AudioCodec};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = CodecConfig::default();
//!     let service = CodecService::new(config).await?;
//!     
//!     // Start a transcoding session
//!     service.start_session(
//!         "session1".to_string(),
//!         AudioCodec::G711Ulaw,
//!         AudioCodec::G711Alaw,
//!         8000,
//!         1
//!     ).await?;
//!     
//!     // ... transcode audio frames ...
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## GPU Acceleration
//!
//! Enable GPU features in Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! redfire-codec-engine = { version = "0.1", features = ["cuda"] }
//! ```

pub mod audio_resampler;
pub mod codec;
pub mod g7222_acelp;
pub mod g729_annex_gpu;
pub mod g729_celp;
pub mod g729_codec;

#[cfg(any(feature = "cuda", feature = "rocm"))]
pub mod gpu_codec_accel;

#[cfg(feature = "cuda")]
pub mod universal_gpu_transcode;

// Re-export main types
pub use codec::{
    AudioCodec, AudioFrame, CodecConfig, CodecService, CodecStatistics, G711Codec, G729Codec,
    OpusCodec, TranscodedFrame, TranscodingSession,
};

pub use audio_resampler::{AudioResampler, ResamplerConfig, ResamplingQuality, ResamplingService};

pub use g729_codec::{G729Frame, G729_ENCODED_SIZE, G729_FRAME_SIZE, G729_SAMPLE_RATE};

pub use g729_celp::{G729Decoder, G729Encoder, L_FRAME, L_SUBFR, M};

pub use g729_annex_gpu::{
    CngState, DtxState, G729AnnexConfig, G729AnnexFrame, G729AnnexGpuProcessor, G729AnnexState,
    G729AnnexStats, G729FrameType, SidFrame, SpectralFeatures, VadResult, VadState,
};

pub use g7222_acelp::{AmrWbMode, G7222Decoder, G7222Encoder, L_FRAME_WB, L_SUBFR_WB, M_WB};

#[cfg(any(feature = "cuda", feature = "rocm"))]
pub use gpu_codec_accel::{
    GpuAccelStats, GpuBackend, GpuBuffer, GpuCodecAccelerator, GpuCodecConfig,
};

#[cfg(feature = "cuda")]
pub use universal_gpu_transcode::UniversalGpuTranscoder;

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

/// Check if GPU acceleration is available
pub fn gpu_available() -> bool {
    cfg!(any(feature = "cuda", feature = "rocm"))
}

/// Get available GPU backends
pub fn available_gpu_backends() -> Vec<&'static str> {
    let backends = Vec::new();

    #[cfg(feature = "cuda")]
    backends.push("cuda");

    #[cfg(feature = "rocm")]
    backends.push("rocm");

    backends
}

/// Create a default codec service with optimal settings
pub async fn create_default_service() -> anyhow::Result<CodecService> {
    let config = CodecConfig::default();
    CodecService::new(config).await
}

/// Create a GPU-accelerated codec service if available
#[cfg(any(feature = "cuda", feature = "rocm"))]
pub async fn create_gpu_service() -> anyhow::Result<CodecService> {
    use crate::codec::CodecConfig;
    use crate::gpu_codec_accel::{GpuBackend, GpuCodecConfig};

    let gpu_config = GpuCodecConfig {
        enabled: true,
        backend: if cfg!(feature = "cuda") {
            GpuBackend::Cuda
        } else {
            GpuBackend::Rocm
        },
        ..Default::default()
    };

    let config = CodecConfig {
        use_gpu: true,
        gpu_config,
        ..Default::default()
    };

    CodecService::new(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_info() {
        assert!(!VERSION.is_empty());
        assert_eq!(NAME, "redfire-codec-engine");
        assert!(!DESCRIPTION.is_empty());
    }

    #[test]
    fn test_gpu_availability() {
        let available = gpu_available();
        let backends = available_gpu_backends();

        if available {
            assert!(!backends.is_empty());
        } else {
            assert!(backends.is_empty());
        }
    }

    #[tokio::test]
    async fn test_default_service_creation() {
        let service = create_default_service().await;
        assert!(service.is_ok());
    }

    #[cfg(any(feature = "cuda", feature = "rocm"))]
    #[tokio::test]
    async fn test_gpu_service_creation() {
        // This test may fail if no GPU is available, which is expected
        let _service = create_gpu_service().await;
        // Don't assert success since GPU may not be available in test environment
    }
}
