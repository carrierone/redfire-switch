# Redfire Switch Documentation

*Automatically generated documentation*

## Table of Contents

1. [Architecture Overview](architecture.md)
2. [API Documentation](api_documentation.md)
3. [Module Dependencies](module_dependencies.mermaid)
4. [Call Flows](call_flows/)
5. [Configuration Examples](../config/)

## Quick Start

```bash
# Build the project
cargo build --release

# Generate default configuration
./target/release/redfire-switch gen-config config.toml

# Start the switch
./target/release/redfire-switch --config config.toml start
```

## Project Statistics

- **Total Modules**: 60
- **Total Lines of Code**: 50410
- **Call Flows Documented**: 4
- **Public Functions**: 761
- **Public Structs**: 526

## Features

- 🚀 **High Performance**: 10,000+ CPS capability
- 📞 **SIP Compliance**: RFC 3261 and extensions
- 🔒 **Security**: STIR/SHAKEN, TLS, authentication
- 🌐 **Interoperability**: Works with all major SIP stacks
- 📊 **Monitoring**: Comprehensive CDR and analytics
- 🚨 **Emergency**: 911/112 call routing
- 📱 **Modern**: IMS/VoLTE support

## Documentation Generation

This documentation was generated using the standalone documentation generator:

```bash
# Run the documentation generator
cargo run --manifest-path scripts/Cargo.toml --bin generate_docs -- --verbose
```

