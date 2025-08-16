# Redfire Switch

A high-performance Class 4 SIP telephone switch written in Rust, designed for modern telecommunications infrastructure with advanced codec transcoding and GPU acceleration.

**Sponsored by [Carrier One Inc](https://www.carrierone.com)**

> *"And the angel of the Lord appeared unto him in a flame of fire out of the midst of a bush: and he looked, and, behold, the bush burned with fire, and the bush was not consumed."* - Exodus 3:2
>
> Like the burning bush that was not consumed, Redfire Switch is designed to burn brightly with reliable telecommunications fire while remaining robust and enduring under any load.

## Features

### Core SIP Switch
- **Complete SIP Stack** - RFC 3261 compliant SIP server
- **Call Routing** - Advanced routing engine with LCR, LNP, and ENUM support
- **Call Detail Records** - Comprehensive CDR with ClickHouse integration
- **STIR/SHAKEN** - JWT-based call authentication and fraud prevention
- **Emergency Services** - 911/E911 routing with location services

### Advanced Codec Engine
- **Universal GPU Transcoding** - Hardware-accelerated codec conversion
- **56 Codec Pairs** - Direct transcoding between all supported formats
- **Real-time Performance** - 15-20x faster than CPU-only solutions
- **Patent-Free Codecs** - G.729, G.722.2/AMR-WB implementations

#### Supported Codecs
| Codec | Description | Sample Rate | GPU Accelerated |
|-------|-------------|-------------|-----------------|
| **G.711 μ-law/A-law** | Standard telephony | 8 kHz | ✅ |
| **G.729/G.729A/G.729B** | CELP compression | 8 kHz | ✅ |
| **G.722.2 (AMR-WB)** | Wideband ACELP | 16 kHz | ✅ |
| **G.722** | ADPCM wideband | 16 kHz | ✅ |
| **PCM16** | Linear PCM | 8/16 kHz | ✅ |
| **Opus** | Modern codec | 48 kHz | CPU |

### Enterprise Features
- **High Availability** - Clustering and failover support
- **Security** - TLS, SRTP, and comprehensive authentication
- **Monitoring** - Prometheus metrics and health checks
- **Regulatory Compliance** - TCPA, TRACED Act, FCC requirements

### AI Integration
- **MCP Server** - Model Context Protocol server for AI developers
- **JSON-RPC API** - RESTful access to all telecommunications functions
- **Tool Integration** - Codec transcoding, SIP operations, call analysis
- **AI-Friendly** - Designed for LLM and automation integration

## Quick Start

### Installation

#### Prerequisites
- **Rust 1.70+** - Latest stable toolchain
- **Docker & Docker Compose** - For development environment
- **CUDA 11.0+** or **ROCm 5.0+** - For GPU acceleration (optional)

#### Development Setup
```bash
# Clone the repository
git clone https://github.com/carrierone/redfire-switch.git
cd redfire-switch

# Setup development environment
./setup-dev.sh

# Build the project
cargo build --release

# Run tests
cargo test
```

#### Docker Development
```bash
# Start development environment
docker-compose -f docker-compose.dev.yml up -d

# Run with GPU support
docker-compose -f docker-compose.dev.yml -f docker-compose.gpu.yml up -d
```

### Configuration

#### Basic Configuration (`config.json`)
```json
{
  "sip": {
    "listen_address": "0.0.0.0:5060",
    "external_ip": "203.0.113.1",
    "transport": ["UDP", "TCP", "TLS"]
  },
  "routing": {
    "default_gateway": "sip:gateway.provider.com",
    "lcr_enabled": true,
    "enum_suffix": "e164.arpa"
  },
  "codec": {
    "enabled": true,
    "use_gpu": true,
    "gpu_device": 0
  }
}
```

#### GPU Acceleration Setup
```bash
# Install GPU development environment
./setup-gpu-dev.sh

# Build with CUDA support
cargo build --features cuda --release

# Build with ROCm support  
cargo build --features rocm --release
```

### Usage Examples

#### Basic SIP Call Routing
```rust
use redfire_switch::{SipServer, RoutingEngine, CodecService};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize services
    let codec_service = CodecService::new(config.codec).await?;
    let routing_engine = RoutingEngine::new(config.routing).await?;
    let sip_server = SipServer::new(config.sip).await?;
    
    // Start processing calls
    sip_server.start().await?;
    
    Ok(())
}
```

#### GPU Codec Transcoding
```rust
use redfire_codec_engine::{UniversalGpuTranscoder, AudioCodec, AudioFrame};

// Initialize GPU transcoder
let transcoder = UniversalGpuTranscoder::new(1000).await?;

// Transcode G.729 to G.722.2 
let frame = AudioFrame {
    data: g729_data,
    codec: AudioCodec::G729,
    sample_rate: 8000,
    channels: 1,
    timestamp: 12345,
    sequence: 1,
};

let result = transcoder.transcode_frame(&frame, AudioCodec::G7222).await?;
println!("Transcoded in {}μs", result.processing_time_us);
```

## Architecture

### System Overview
```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ SIP Clients │────│ Redfire     │────│ Telecom     │
│             │    │ Switch      │    │ Providers   │
└─────────────┘    └─────────────┘    └─────────────┘
                          │
                   ┌─────────────┐
                   │ GPU Codec   │
                   │ Engine      │
                   └─────────────┘
```

### Component Architecture
- **SIP Stack** (`redfire-sip-stack`) - Core SIP protocol handling
- **Codec Engine** (`redfire-codec-engine`) - Audio transcoding with GPU acceleration
- **Routing Engine** - Call routing, LCR, and number manipulation
- **Media Plane** - RTP handling and codec negotiation
- **Control Plane** - SIP signaling and call control

## Performance

### Codec Transcoding Benchmarks
| Operation | CPU (μs) | GPU (μs) | Speedup |
|-----------|----------|----------|---------|
| G.711 μ-law → A-law | 12 | 0.8 | **15x** |
| G.729 → G.711 | 850 | 45 | **19x** |
| G.722.2 → G.711 | 920 | 55 | **17x** |
| G.729 → G.722.2 | 1200 | 75 | **16x** |

### Scalability
- **Single Server**: 10,000+ concurrent calls
- **GPU Acceleration**: 15,000x realtime transcoding
- **Memory Usage**: 3.2 KB per stream (GPU) vs 8 KB (CPU)
- **Latency**: <50μs transcoding delay

## Testing

### Automated Testing
```bash
# Run unit tests
cargo test

# Run integration tests
./test-libraries.sh

# Run SIP compliance tests
./test-rfc-compliance.sh

# Performance benchmarks
cargo bench --features cuda
```

### SIPp Testing
```bash
# Basic call test
sipp -sf scenarios/basic_call_uac.xml target_ip:5060

# Stress test
sipp -sf scenarios/stress_test.xml -r 100 -d 10000 target_ip:5060

# Codec transcoding test
sipp -sf scenarios/codec_test.xml target_ip:5060
```

## Documentation

- **[Installation Guide](INSTALLATION.md)** - Detailed setup instructions
- **[Configuration Reference](CONFIGURATION.md)** - Complete configuration options
- **[Testing Guide](TESTING.md)** - Testing procedures and scenarios
- **[Architecture Guide](ARCHITECTURE.md)** - System design and components
- **[Deployment Guide](DEPLOYMENT-GUIDE.md)** - Production deployment

## Library Usage

### Codec Engine as Library
```toml
[dependencies]
redfire-codec-engine = { git = "https://github.com/carrierone/redfire-switch", features = ["cuda"] }
```

### SIP Stack as Library  
```toml
[dependencies]
redfire-sip-stack = { git = "https://github.com/carrierone/redfire-switch" }
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Environment
```bash
# Setup pre-commit hooks
pre-commit install

# Run linting
cargo clippy --all-targets --all-features

# Format code
cargo fmt --all
```

## License

This project is licensed under the GNU General Public License v3.0 or later - see the [LICENSE](LICENSE) file for details.

## Commercial Support

For commercial support, licensing, and enterprise deployments, contact [Carrier One Inc](https://www.carrierone.com).

---

**Built with ❤️ in Rust for the telecommunications industry**