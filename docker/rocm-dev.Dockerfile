# ROCm Development Environment for Redfire Switch
FROM rocm/dev-ubuntu-22.04:5.7

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Set ROCm environment variables
ENV ROCM_PATH=/opt/rocm
ENV HIP_PATH=/opt/rocm
ENV PATH=${ROCM_PATH}/bin:${PATH}
ENV LD_LIBRARY_PATH=${ROCM_PATH}/lib:${LD_LIBRARY_PATH}

# Verify ROCm installation
RUN hipcc --version

# Set working directory
WORKDIR /workspace

# Copy project files
COPY . .

# Build with ROCm support
RUN cargo build --features rocm

CMD ["/bin/bash"]