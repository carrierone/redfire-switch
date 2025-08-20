# Redfire Switch Documentation

*Automatically generated documentation*

## Table of Contents

### Core Documentation
1. [Architecture Overview](architecture.md)
2. [API Documentation](api_documentation.md)
3. [Module Dependencies - System Overview](module_dependencies.mermaid)
4. [Module Dependencies - Core Components](module_dependencies_core.mermaid)
5. [Module Dependencies - SIP Stack](module_dependencies_sip.mermaid)  
6. [Module Dependencies - Routing Engine](module_dependencies_routing.mermaid)
7. [Module Dependencies - Media Processing](module_dependencies_media.mermaid)
8. [Call Flows](call_flows/)
9. [Configuration Examples](../config/)

### Lawful Intercept Documentation
10. [Lawful Intercept Implementation Guide](../LAWFUL_INTERCEPT_GUIDE.md)
11. [LI Technical Setup Guide](../LI_TECHNICAL_SETUP.md)
12. [LI Compliance & Security Guide](../LI_COMPLIANCE_SECURITY.md)
13. [LI System Architecture](lawful_intercept_architecture.mermaid)
14. [LI Data Flow Diagram](li_data_flow.mermaid)
15. [LI Operational Overview](li_operational_overview.mermaid)

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

### Core Telecommunications
- 🚀 **High Performance**: 10,000+ CPS capability
- 📞 **SIP Compliance**: RFC 3261 and extensions
- 🔒 **Security**: STIR/SHAKEN, TLS, authentication
- 🌐 **Interoperability**: Works with all major SIP stacks
- 📊 **Monitoring**: Comprehensive CDR and analytics
- 🚨 **Emergency**: 911/112 call routing
- 📱 **Modern**: IMS/VoLTE support

### Lawful Intercept & Compliance
- ⚖️ **ETSI LI**: European lawful intercept standards (TS 102 232, TS 133 108)
- 🇺🇸 **CALEA**: U.S. lawful intercept compliance (J-STD-025)
- 🔐 **HI2/HI3**: Secure intercept-related information and content delivery
- 📋 **CDR Generation**: Real-time call detail records with lawful intercept fields
- 🛡️ **Warrant Management**: Court order validation and lifecycle management
- 🔒 **Data Encryption**: End-to-end encryption with HSM support
- 📊 **Audit Trail**: Comprehensive tamper-evident logging
- 🚨 **LEA Delivery**: Secure data delivery to Law Enforcement Agencies

## Architecture Diagrams

The system architecture is documented through multiple focused diagrams to stay within GitHub's 500-edge Mermaid limit:

- **System Overview**: High-level components and data flow
- **Core Components**: Main application architecture 
- **SIP Stack**: Protocol implementation details
- **Routing Engine**: Call routing logic and services
- **Media Processing**: Audio/video pipeline

Each diagram contains 20-40 nodes for optimal viewing and GitHub compatibility.

## Documentation Generation

This documentation was generated using the standalone documentation generator:

```bash
# Run the documentation generator
cargo run --manifest-path scripts/Cargo.toml --bin generate_docs -- --verbose
```

