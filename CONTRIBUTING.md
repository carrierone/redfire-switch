# Contributing to Redfire Switch

We welcome contributions to Redfire Switch! This document provides guidelines for contributing to the project.

## Development Environment

### Prerequisites
- **Rust 1.70+** - Latest stable toolchain
- **Docker & Docker Compose** - For testing environment
- **Git** - Version control
- **CUDA 11.0+** or **ROCm 5.0+** - For GPU development (optional)

### Setup
```bash
# Clone the repository
git clone https://github.com/carrierone/redfire-switch.git
cd redfire-switch

# Setup development environment
./setup-dev.sh

# Install pre-commit hooks (optional but recommended)
pre-commit install
```

### Build and Test
```bash
# Build the project
cargo build

# Run tests
cargo test

# Run with GPU features
cargo test --features cuda

# Run integration tests
./test-libraries.sh
```

## Code Style

### Formatting
```bash
# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check
```

### Linting
```bash
# Run clippy
cargo clippy --all-targets --all-features

# Fix clippy warnings
cargo clippy --all-targets --all-features --fix
```

## Testing

### Unit Tests
- Write unit tests for all new functionality
- Place tests in the same file as the code using `#[cfg(test)]`
- Use descriptive test names that explain what is being tested

### Integration Tests
- Add integration tests for new features in the `tests/` directory
- Test end-to-end functionality and API contracts
- Include performance benchmarks for codec transcoding

### GPU Tests
- Test both CUDA and ROCm backends when applicable
- Include fallback testing (GPU failure scenarios)
- Validate quality metrics for audio processing

## Pull Request Process

### Before Submitting
1. **Run Tests**: Ensure all tests pass
   ```bash
   cargo test --all-features
   ./test-libraries.sh
   ```

2. **Format Code**: Apply consistent formatting
   ```bash
   cargo fmt --all
   ```

3. **Fix Warnings**: Address all clippy warnings
   ```bash
   cargo clippy --all-targets --all-features
   ```

4. **Update Documentation**: Update relevant documentation

### PR Guidelines
- **Clear Title**: Describe what the PR does
- **Detailed Description**: Explain the changes and why they're needed
- **Link Issues**: Reference any related issues
- **Small Changes**: Keep PRs focused and reasonably sized
- **Tests Included**: Add tests for new functionality

### Review Process
1. All PRs require review before merging
2. CI must pass (builds, tests, linting)
3. Address review feedback promptly
4. Maintain a clean commit history

## Code Organization

### Project Structure
```
redfire-switch/
├── src/                    # Main application code
├── redfire-codec-engine/   # Codec transcoding library
├── redfire-sip-stack/      # SIP protocol library
├── tests/                  # Integration tests
├── docs/                   # Documentation
└── examples/               # Usage examples
```

### Module Guidelines
- **Single Responsibility**: Each module should have a clear purpose
- **Clear APIs**: Expose well-defined public interfaces
- **Error Handling**: Use `Result<T, E>` for fallible operations
- **Documentation**: Document all public APIs with examples

## Feature Development

### New Codecs
When adding new codec support:

1. **Research**: Ensure patent-free implementation
2. **Standards**: Follow relevant ITU-T or RFC specifications
3. **Testing**: Include quality validation tests
4. **GPU Support**: Add GPU acceleration if feasible
5. **Documentation**: Update codec support tables

### GPU Kernels
For new GPU functionality:

1. **CUDA/ROCm**: Support both backends when possible
2. **Error Handling**: Implement graceful CPU fallback
3. **Memory Management**: Use memory pools efficiently
4. **Testing**: Validate against CPU reference implementation

### SIP Features
For SIP protocol enhancements:

1. **RFC Compliance**: Follow SIP RFCs strictly
2. **Interoperability**: Test with multiple SIP implementations
3. **Security**: Consider security implications
4. **Documentation**: Update configuration examples

## Documentation

### Code Documentation
- **Public APIs**: Document all public functions and types
- **Examples**: Include usage examples in doc comments
- **Error Conditions**: Document when functions can fail

### User Documentation
- **Installation**: Update installation guides for new requirements
- **Configuration**: Document new configuration options
- **Examples**: Provide working code examples

## Issue Reporting

### Bug Reports
Include:
- **Environment**: OS, Rust version, GPU drivers
- **Steps to Reproduce**: Clear reproduction steps
- **Expected vs Actual**: What should happen vs what does happen
- **Logs**: Relevant log output (sanitized)

### Feature Requests
Include:
- **Use Case**: Why is this feature needed?
- **Proposal**: How should it work?
- **Alternatives**: What alternatives were considered?

## Community Guidelines

### Communication
- **Be Respectful**: Treat everyone with respect
- **Be Constructive**: Provide helpful feedback
- **Be Patient**: Allow time for responses
- **Be Inclusive**: Welcome contributors of all backgrounds

### Code of Conduct
We are committed to providing a welcoming and inclusive environment. By participating in this project, you agree to abide by our Code of Conduct.

## License

By contributing to Redfire Switch, you agree that your contributions will be licensed under the GNU General Public License v3.0 or later.

## Questions?

- **GitHub Issues**: For bugs and feature requests
- **GitHub Discussions**: For questions and general discussion
- **Email**: For security issues or commercial inquiries: info@carrierone.com

Thank you for contributing to Redfire Switch! 🚀