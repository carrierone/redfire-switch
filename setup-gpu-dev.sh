#!/bin/bash

# GPU Development Environment Setup for Redfire Switch
# Copyright (C) 2025 Carrier One Inc and contributors

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "\n${BLUE}=== $1 ===${NC}"
}

# Check system information
check_system() {
    print_header "System Information"
    
    echo "OS: $(lsb_release -d | cut -f2)"
    echo "Kernel: $(uname -r)"
    echo "Architecture: $(uname -m)"
    
    # Check for NVIDIA GPU
    if lspci | grep -qi nvidia; then
        print_info "NVIDIA GPU detected:"
        lspci | grep -i nvidia
        HAS_NVIDIA=true
    else
        print_info "No NVIDIA GPU detected"
        HAS_NVIDIA=false
    fi
    
    # Check for AMD GPU
    if lspci | grep -qi amd; then
        print_info "AMD GPU detected:"
        lspci | grep -i amd
        HAS_AMD=true
    else
        print_info "No AMD GPU detected"
        HAS_AMD=false
    fi
}

# Install CUDA
install_cuda() {
    print_header "Installing CUDA"
    
    if ! $HAS_NVIDIA; then
        print_warning "No NVIDIA GPU detected, skipping CUDA installation"
        return 0
    fi
    
    # Check if CUDA is already installed
    if command -v nvcc >/dev/null 2>&1; then
        print_info "CUDA already installed: $(nvcc --version | grep release)"
        return 0
    fi
    
    print_info "Installing CUDA toolkit..."
    
    # Detect Ubuntu version
    UBUNTU_VERSION=$(lsb_release -rs | tr -d '.')
    
    case $UBUNTU_VERSION in
        2204)
            CUDA_REPO="ubuntu2204"
            ;;
        2004)
            CUDA_REPO="ubuntu2004"
            ;;
        1804)
            CUDA_REPO="ubuntu1804"
            ;;
        *)
            print_error "Unsupported Ubuntu version: $UBUNTU_VERSION"
            return 1
            ;;
    esac
    
    # Add CUDA repository
    wget -q https://developer.download.nvidia.com/compute/cuda/repos/$CUDA_REPO/x86_64/cuda-keyring_1.0-1_all.deb
    sudo dpkg -i cuda-keyring_1.0-1_all.deb
    sudo apt-get update
    
    # Install CUDA toolkit
    sudo apt-get install -y cuda-toolkit-12-3
    
    # Add to PATH
    echo 'export PATH=/usr/local/cuda/bin:$PATH' >> ~/.bashrc
    echo 'export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH' >> ~/.bashrc
    echo 'export CUDA_HOME=/usr/local/cuda' >> ~/.bashrc
    
    # Source for current session
    export PATH=/usr/local/cuda/bin:$PATH
    export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH
    export CUDA_HOME=/usr/local/cuda
    
    print_success "CUDA installed successfully"
}

# Install ROCm
install_rocm() {
    print_header "Installing ROCm"
    
    if ! $HAS_AMD; then
        print_warning "No AMD GPU detected, skipping ROCm installation"
        return 0
    fi
    
    # Check if ROCm is already installed
    if command -v hipcc >/dev/null 2>&1; then
        print_info "ROCm already installed: $(hipcc --version | head -1)"
        return 0
    fi
    
    print_info "Installing ROCm..."
    
    # Add ROCm repository
    sudo mkdir -p /etc/apt/keyrings
    wget -q https://repo.radeon.com/rocm/rocm.gpg.key -O - | gpg --dearmor | sudo tee /etc/apt/keyrings/rocm.gpg > /dev/null
    
    echo "deb [arch=amd64 signed-by=/etc/apt/keyrings/rocm.gpg] https://repo.radeon.com/rocm/apt/5.7 jammy main" | sudo tee /etc/apt/sources.list.d/rocm.list
    
    sudo apt-get update
    
    # Install ROCm packages
    sudo apt-get install -y rocm-dev rocm-libs hip-dev
    
    # Add user to groups
    sudo usermod -a -G render,video $USER
    
    # Add to PATH
    echo 'export ROCM_PATH=/opt/rocm' >> ~/.bashrc
    echo 'export PATH=$ROCM_PATH/bin:$PATH' >> ~/.bashrc
    echo 'export LD_LIBRARY_PATH=$ROCM_PATH/lib:$LD_LIBRARY_PATH' >> ~/.bashrc
    echo 'export HIP_PATH=$ROCM_PATH' >> ~/.bashrc
    
    # Source for current session
    export ROCM_PATH=/opt/rocm
    export PATH=$ROCM_PATH/bin:$PATH
    export LD_LIBRARY_PATH=$ROCM_PATH/lib:$LD_LIBRARY_PATH
    export HIP_PATH=$ROCM_PATH
    
    print_success "ROCm installed successfully"
    print_warning "Please reboot your system to complete ROCm installation"
}

# Install Rust and dependencies
install_rust_deps() {
    print_header "Installing Rust and Dependencies"
    
    # Install system dependencies
    print_info "Installing system dependencies..."
    sudo apt-get update
    sudo apt-get install -y \
        build-essential \
        pkg-config \
        libssl-dev \
        git \
        curl \
        wget \
        cmake \
        ninja-build
    
    # Install Rust if not present
    if ! command -v cargo >/dev/null 2>&1; then
        print_info "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
    else
        print_info "Rust already installed: $(rustc --version)"
    fi
    
    print_success "Rust and dependencies installed"
}

# Test GPU setup
test_gpu_setup() {
    print_header "Testing GPU Setup"
    
    # Test CUDA
    if command -v nvcc >/dev/null 2>&1; then
        print_info "Testing CUDA..."
        nvcc --version
        
        if command -v nvidia-smi >/dev/null 2>&1; then
            nvidia-smi
        fi
        
        # Test simple CUDA compilation
        cat > /tmp/test_cuda.cu << EOF
#include <stdio.h>

__global__ void hello() {
    printf("Hello from GPU!\n");
}

int main() {
    hello<<<1,1>>>();
    cudaDeviceSynchronize();
    return 0;
}
EOF
        
        if nvcc /tmp/test_cuda.cu -o /tmp/test_cuda 2>/dev/null; then
            print_success "CUDA compilation test passed"
            if /tmp/test_cuda 2>/dev/null; then
                print_success "CUDA runtime test passed"
            else
                print_warning "CUDA runtime test failed"
            fi
        else
            print_warning "CUDA compilation test failed"
        fi
        
        rm -f /tmp/test_cuda.cu /tmp/test_cuda
    fi
    
    # Test ROCm
    if command -v hipcc >/dev/null 2>&1; then
        print_info "Testing ROCm..."
        hipcc --version
        
        if command -v rocm-smi >/dev/null 2>&1; then
            rocm-smi
        fi
        
        # Test simple HIP compilation
        cat > /tmp/test_hip.cpp << EOF
#include <hip/hip_runtime.h>
#include <stdio.h>

__global__ void hello() {
    printf("Hello from GPU!\n");
}

int main() {
    hello<<<1,1>>>();
    hipDeviceSynchronize();
    return 0;
}
EOF
        
        if hipcc /tmp/test_hip.cpp -o /tmp/test_hip 2>/dev/null; then
            print_success "HIP compilation test passed"
            if /tmp/test_hip 2>/dev/null; then
                print_success "HIP runtime test passed"
            else
                print_warning "HIP runtime test failed"
            fi
        else
            print_warning "HIP compilation test failed"
        fi
        
        rm -f /tmp/test_hip.cpp /tmp/test_hip
    fi
}

# Test Redfire libraries with GPU
test_redfire_gpu() {
    print_header "Testing Redfire Libraries with GPU Support"
    
    # Test CUDA build
    if command -v nvcc >/dev/null 2>&1; then
        print_info "Testing Redfire codec engine with CUDA..."
        cd redfire-codec-engine
        if cargo build --features cuda; then
            print_success "Redfire codec engine builds with CUDA"
            if cargo test --features cuda; then
                print_success "Redfire codec engine CUDA tests passed"
            else
                print_warning "Redfire codec engine CUDA tests failed"
            fi
        else
            print_error "Redfire codec engine CUDA build failed"
        fi
        cd ..
    fi
    
    # Test ROCm build
    if command -v hipcc >/dev/null 2>&1; then
        print_info "Testing Redfire codec engine with ROCm..."
        cd redfire-codec-engine
        if cargo build --features rocm; then
            print_success "Redfire codec engine builds with ROCm"
            if cargo test --features rocm; then
                print_success "Redfire codec engine ROCm tests passed"
            else
                print_warning "Redfire codec engine ROCm tests failed"
            fi
        else
            print_error "Redfire codec engine ROCm build failed"
        fi
        cd ..
    fi
}

# Create development scripts
create_dev_scripts() {
    print_header "Creating Development Scripts"
    
    # Create build script with GPU detection
    cat > build-with-gpu.sh << 'EOF'
#!/bin/bash
# Auto-detect and build with GPU support

if command -v nvcc >/dev/null 2>&1; then
    echo "Building with CUDA support..."
    cargo build --features cuda "$@"
elif command -v hipcc >/dev/null 2>&1; then
    echo "Building with ROCm support..."
    cargo build --features rocm "$@"
else
    echo "Building without GPU support..."
    cargo build "$@"
fi
EOF
    chmod +x build-with-gpu.sh
    
    # Create test script with GPU detection
    cat > test-with-gpu.sh << 'EOF'
#!/bin/bash
# Auto-detect and test with GPU support

if command -v nvcc >/dev/null 2>&1; then
    echo "Testing with CUDA support..."
    cargo test --features cuda "$@"
elif command -v hipcc >/dev/null 2>&1; then
    echo "Testing with ROCm support..."
    cargo test --features rocm "$@"
else
    echo "Testing without GPU support..."
    cargo test "$@"
fi
EOF
    chmod +x test-with-gpu.sh
    
    print_success "Development scripts created: build-with-gpu.sh, test-with-gpu.sh"
}

# Main function
main() {
    print_header "Redfire Switch GPU Development Environment Setup"
    
    echo "This script will set up CUDA and/or ROCm development environment"
    echo "for the Redfire Switch libraries."
    echo ""
    
    # Parse command line arguments
    INSTALL_CUDA=false
    INSTALL_ROCM=false
    FORCE=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --cuda)
                INSTALL_CUDA=true
                shift
                ;;
            --rocm)
                INSTALL_ROCM=true
                shift
                ;;
            --all)
                INSTALL_CUDA=true
                INSTALL_ROCM=true
                shift
                ;;
            --force)
                FORCE=true
                shift
                ;;
            --help)
                echo "Usage: $0 [--cuda] [--rocm] [--all] [--force] [--help]"
                echo "  --cuda    Install CUDA toolkit"
                echo "  --rocm    Install ROCm platform"
                echo "  --all     Install both CUDA and ROCm"
                echo "  --force   Force installation even if GPU not detected"
                echo "  --help    Show this help"
                exit 0
                ;;
            *)
                print_error "Unknown argument: $1"
                exit 1
                ;;
        esac
    done
    
    # Check system
    check_system
    
    # Install Rust and dependencies
    install_rust_deps
    
    # Auto-detect if no explicit choice
    if ! $INSTALL_CUDA && ! $INSTALL_ROCM; then
        if $HAS_NVIDIA; then
            INSTALL_CUDA=true
        fi
        if $HAS_AMD; then
            INSTALL_ROCM=true
        fi
    fi
    
    # Install GPU SDKs
    if $INSTALL_CUDA; then
        if $HAS_NVIDIA || $FORCE; then
            install_cuda
        else
            print_warning "No NVIDIA GPU detected, use --force to install anyway"
        fi
    fi
    
    if $INSTALL_ROCM; then
        if $HAS_AMD || $FORCE; then
            install_rocm
        else
            print_warning "No AMD GPU detected, use --force to install anyway"
        fi
    fi
    
    # Test setup
    test_gpu_setup
    
    # Test Redfire libraries
    test_redfire_gpu
    
    # Create development scripts
    create_dev_scripts
    
    print_header "Setup Complete"
    print_success "GPU development environment setup completed!"
    
    if $INSTALL_ROCM && $HAS_AMD; then
        print_warning "Please reboot your system to complete ROCm installation"
    fi
    
    echo ""
    echo "Next steps:"
    echo "1. Source your shell configuration: source ~/.bashrc"
    echo "2. Test GPU builds: ./build-with-gpu.sh"
    echo "3. Run GPU tests: ./test-with-gpu.sh"
    echo "4. Use GPU features: cargo build --features cuda (or rocm)"
}

# Run main function
main "$@"