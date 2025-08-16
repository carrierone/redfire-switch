# Redfire Switch Libraries

This document describes how to build, install, and use the Redfire Switch libraries as standalone components.

## Overview

The Redfire Switch codebase has been refactored to extract core functionality into reusable libraries:

### 📦 redfire-codec-engine
A professional audio codec translation engine with GPU acceleration support.

**Features:**
- Multiple audio codec support (G.711, G.729, Opus, G.722, PCM)
- GPU-accelerated transcoding (CUDA/ROCm)
- Professional audio resampling
- G.729 Annex A/B with VAD, DTX, and CNG
- Real-time performance optimizations
- Memory pooling for efficient resource usage

### 📦 redfire-sip-stack
Complete SIP, SIP-I, and SIP-T protocol stack implementation.

**Features:**
- Complete SIP message parsing and validation
- SIP state machine and transaction handling
- Multiple authentication mechanisms
- Multi-transport support (UDP/TCP/TLS)
- SIP-T multipart MIME with ISUP encapsulation
- SIP-I ISUP message handling
- RFC compliance checking
- Interoperability testing tools

## Quick Start

### Option 1: Using the Installation Script (Recommended)

```bash
# Install to /usr/local (requires sudo)
./install-libraries.sh

# Install to custom location
./install-libraries.sh --prefix /opt/redfire

# Debug build with tests
./install-libraries.sh --debug --test

# Install to user directory (no sudo required)
PREFIX=$HOME/.local ./install-libraries.sh
```

### Option 2: Using Make

```bash
# Build both libraries
make -f Makefile.libs all

# Install to system location (requires sudo)
sudo make -f Makefile.libs install

# Install to custom prefix
make -f Makefile.libs install PREFIX=/opt/redfire

# Debug build
make -f Makefile.libs BUILD_TYPE=debug all

# With GPU support (auto-detected)
make -f Makefile.libs FEATURES=cuda all
```

### Option 3: Manual Cargo Build

```bash
# Build codec engine
cd redfire-codec-engine
cargo build --release --features cuda
cd ..

# Build SIP stack
cd redfire-sip-stack
cargo build --release
cd ..
```

## Usage Examples

### Using the Codec Engine

```rust
use redfire_codec_engine::{
    AudioCodec, AudioFrame, CodecConfig, CodecService, 
    create_default_service,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create codec service
    let service = create_default_service().await?;
    
    // Start transcoding session
    service.start_session(
        "session1".to_string(),
        AudioCodec::G711Ulaw,
        AudioCodec::G711Alaw,
        8000, 1
    ).await?;
    
    // Create and transcode audio frame
    let frame = AudioFrame {
        data: vec![0x7F; 160], // μ-law data
        codec: AudioCodec::G711Ulaw,
        sample_rate: 8000,
        channels: 1,
        timestamp: 0,
        sequence: 1,
    };
    
    let transcoded = service.transcode_frame("session1", frame).await?;
    println!("Transcoded to {:?}", transcoded.target_codec);
    
    Ok(())
}
```

### Using the SIP Stack

```rust
use redfire_sip_stack::{
    SipParser, create_default_parser, utils,
    create_sipt_sipi_service,
};

fn main() -> anyhow::Result<()> {
    // Parse SIP message
    let parser = create_default_parser();
    let sip_data = "INVITE sip:alice@example.com SIP/2.0\r\n...";
    let message = parser.parse_message(sip_data.as_bytes())?;
    
    // Use SIP utilities
    let call_id = utils::generate_call_id();
    let is_valid = utils::validate_sip_uri("sip:alice@example.com");
    
    // SIP-T/SIP-I support
    let sipt_service = create_sipt_sipi_service();
    
    Ok(())
}
```

## GPU Acceleration

### CUDA Support

Ensure CUDA toolkit is installed:
```bash
# Check CUDA availability
nvcc --version

# Build with CUDA support
./install-libraries.sh  # Auto-detects CUDA
# or
make -f Makefile.libs FEATURES=cuda all
```

### ROCm Support

Ensure ROCm is installed:
```bash
# Check ROCm availability
hipcc --version

# Build with ROCm support
make -f Makefile.libs FEATURES=rocm all
```

## Integration with pkg-config

After installation, the libraries can be used in other projects:

```bash
# Get compiler flags
pkg-config --cflags redfire-codec-engine
pkg-config --cflags redfire-sip-stack

# Get linker flags
pkg-config --libs redfire-codec-engine
pkg-config --libs redfire-sip-stack
```

### CMake Integration

```cmake
find_package(PkgConfig REQUIRED)
pkg_check_modules(REDFIRE_CODEC REQUIRED redfire-codec-engine)
pkg_check_modules(REDFIRE_SIP REQUIRED redfire-sip-stack)

target_link_libraries(your_target 
    ${REDFIRE_CODEC_LIBRARIES} 
    ${REDFIRE_SIP_LIBRARIES}
)
```

### Rust Integration

Add to your `Cargo.toml`:

```toml
[dependencies]
redfire-codec-engine = "0.1"
redfire-sip-stack = "0.1"
```

Or use local path during development:

```toml
[dependencies]
redfire-codec-engine = { path = "../redfire-switch/redfire-codec-engine" }
redfire-sip-stack = { path = "../redfire-switch/redfire-sip-stack" }
```

## Testing

```bash
# Run all library tests
./test-libraries.sh

# Test individual libraries
cd redfire-codec-engine && cargo test
cd redfire-sip-stack && cargo test

# Test with GPU features
cd redfire-codec-engine && cargo test --features cuda
```

## Uninstallation

```bash
# Using make
sudo make -f Makefile.libs uninstall

# Manual removal
sudo rm -f /usr/local/lib/libredfire_*
sudo rm -f /usr/local/lib/pkgconfig/redfire-*
```

## Available Targets and Scripts

### Scripts
- `install-libraries.sh` - Main installation script
- `test-libraries.sh` - Test library compilation
- `Makefile.libs` - Make-based build system

### Make Targets
```bash
make -f Makefile.libs help          # Show all targets
make -f Makefile.libs all           # Build both libraries
make -f Makefile.libs install       # Install libraries
make -f Makefile.libs test          # Run tests
make -f Makefile.libs clean         # Clean build artifacts
make -f Makefile.libs uninstall     # Remove installed libraries
make -f Makefile.libs doc           # Generate documentation
make -f Makefile.libs package       # Create distribution package
```

## System Requirements

### Basic Requirements
- Rust 1.70.0 or later
- pkg-config
- OpenSSL development libraries

### For GPU Acceleration
- CUDA Toolkit 11.0+ (for CUDA support)
- ROCm 5.0+ (for ROCm support)
- Compatible GPU hardware

### Platform Support
- Linux (x86_64, aarch64)
- macOS (x86_64, aarch64)
- Windows (x86_64)

## Contributing

When contributing to the libraries:

1. Maintain backward compatibility in public APIs
2. Update version numbers appropriately
3. Add tests for new functionality
4. Update documentation
5. Ensure cross-platform compatibility

## License

The libraries are licensed under GPL-3.0-or-later, same as the main project.

## Support

For support with the libraries:
- Create issues in the main repository
- Use the SIP debug CLI for SIP stack issues
- Check GPU compatibility for codec engine issues

---

**Note**: These libraries are extracted from the main Redfire Switch application and are designed to be used independently in other telecom and VoIP applications.