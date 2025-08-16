# Redfire Switch Scripts

This directory contains standalone scripts and tools for the Redfire Switch project.

## Documentation Generator

The `generate_docs.rs` script automatically generates comprehensive project documentation.

### Usage

```bash
# Run with default settings
cargo run --manifest-path scripts/Cargo.toml --bin generate_docs

# Run with custom output directory and verbose output
cargo run --manifest-path scripts/Cargo.toml --bin generate_docs -- \
  --output-dir docs/generated \
  --verbose

# Generate PDF documentation (requires pandoc)
cargo run --manifest-path scripts/Cargo.toml --bin generate_docs -- \
  --pdf \
  --verbose

# Run with specific features enabled/disabled
cargo run --manifest-path scripts/Cargo.toml --bin generate_docs -- \
  --no-call-flows \
  --no-module-diagrams \
  --plantuml \
  --mermaid
```

### Command Line Options

- `--project-root <PATH>`: Project root directory (default: current directory)
- `--output-dir <PATH>`: Output directory for generated docs (default: docs/generated)
- `--call-flows`: Generate call flow diagrams (default: true)
- `--module-diagrams`: Generate module dependency diagrams (default: true)
- `--api-docs`: Generate API documentation (default: true)
- `--architecture-docs`: Generate architecture documentation (default: true)
- `--plantuml`: Use PlantUML for diagrams (default: true)
- `--mermaid`: Use Mermaid for diagrams (default: true)
- `--code-examples`: Include source code examples (default: true)
- `--pdf`: Generate PDF output (requires pandoc)
- `--verbose`: Enable verbose output

### Output

The generator creates:

1. **README.md** - Main documentation file with table of contents
2. **architecture.md** - Architecture overview and component descriptions
3. **api_documentation.md** - Module API documentation
4. **module_dependencies.mermaid** - Mermaid module dependency diagram
5. **module_dependencies.puml** - PlantUML module dependency diagram
6. **call_flows/** - Directory containing call flow diagrams:
   - `basic_sip_call.mermaid` / `basic_sip_call.puml`
   - `enum-based_call_routing.mermaid` / `enum-based_call_routing.puml`
   - `emergency_call_(911).mermaid` / `emergency_call_(911).puml`
   - `stir/shaken_call_verification.mermaid` / `stir/shaken_call_verification.puml`
7. **redfire_switch_documentation.pdf** - PDF version (if --pdf is used)

### Requirements

- Rust 1.70+ with tokio async runtime
- Optional: pandoc for PDF generation

### Features

- **Automatic Module Discovery**: Scans the src/ directory for Rust modules
- **Dependency Analysis**: Analyzes use statements to build dependency graphs
- **Call Flow Documentation**: Documents standard telecommunications call flows
- **Multiple Diagram Formats**: Supports both Mermaid and PlantUML
- **Comprehensive Coverage**: API docs, architecture, module structure
- **RFC Compliance Tracking**: Documents implemented RFCs and compliance status
- **Statistics Generation**: Lines of code, module counts, function counts

### Integration

This script is designed to be run as part of CI/CD pipelines or development workflows:

```bash
# In your build pipeline
cd /path/to/redfire-switch
cargo run --manifest-path scripts/Cargo.toml --bin generate_docs -- --verbose
```

The generated documentation provides a complete overview of the Redfire Switch architecture, suitable for developers, operators, and technical documentation.