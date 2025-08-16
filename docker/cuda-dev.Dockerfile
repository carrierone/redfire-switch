# CUDA Development Environment for Redfire Switch
FROM nvidia/cuda:12.3-devel-ubuntu22.04

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

# Set CUDA environment variables
ENV CUDA_HOME=/usr/local/cuda
ENV PATH=${CUDA_HOME}/bin:${PATH}
ENV LD_LIBRARY_PATH=${CUDA_HOME}/lib64:${LD_LIBRARY_PATH}

# Verify CUDA installation
RUN nvcc --version

# Set working directory
WORKDIR /workspace

# Copy project files
COPY . .

# Build with CUDA support
RUN cargo build --features cuda

CMD ["/bin/bash"]