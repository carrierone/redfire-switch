/*
 * Build script for Redfire Codec Engine
 * Handles CUDA kernel compilation and GPU feature detection
 */

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/");

    // Check for CUDA feature
    #[cfg(feature = "cuda")]
    {
        println!("cargo:rerun-if-changed=src/g729_g711_direct_transcode.cu");
        println!("cargo:rerun-if-changed=src/universal_codec_transcode.cu");
        compile_cuda_kernels();
    }

    // Check for ROCm feature
    #[cfg(feature = "rocm")]
    {
        setup_rocm_compilation();
    }

    // Generate version info
    generate_version_info();
}

#[cfg(feature = "cuda")]
fn compile_cuda_kernels() {
    println!("cargo:rustc-link-lib=cuda");
    println!("cargo:rustc-link-lib=cudart");

    // Find CUDA installation
    let cuda_path = find_cuda_installation();
    if cuda_path.is_none() {
        println!("cargo:warning=CUDA installation not found, skipping kernel compilation");
        return;
    }

    let cuda_path = cuda_path.expect("CUDA path should be validated before calling this function");
    println!(
        "cargo:rustc-link-search=native={}/lib64",
        cuda_path.display()
    );

    // Compile CUDA kernels
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR environment variable should be set by Cargo");
    let kernel_sources = [
        "src/g729_g711_direct_transcode.cu",
        "src/universal_codec_transcode.cu",
    ];

    for kernel_source in &kernel_sources {
        if !std::path::Path::new(kernel_source).exists() {
            println!(
                "cargo:warning=CUDA kernel source not found: {}",
                kernel_source
            );
            continue;
        }
        let nvcc_path = cuda_path.join("bin").join("nvcc");
        let kernel_name = std::path::Path::new(kernel_source)
            .file_stem()
            .expect("Kernel source path should have a filename")
            .to_string_lossy();
        let output_path = format!("{}/{}.ptx", out_dir, kernel_name);

        let output = Command::new(&nvcc_path)
            .args(&[
                "--ptx",
                "--gpu-architecture=sm_50", // Minimum compatible architecture
                "--relocatable-device-code=true",
                "--output-file",
                &output_path,
                kernel_source,
            ])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    println!(
                        "cargo:rustc-env=CUDA_KERNEL_{}={}",
                        kernel_name.to_uppercase().replace("-", "_"),
                        output_path
                    );
                    println!(
                        "Successfully compiled CUDA kernel {} to {}",
                        kernel_name, output_path
                    );
                } else {
                    println!(
                        "cargo:warning=CUDA kernel compilation failed for {}: {}",
                        kernel_name,
                        String::from_utf8_lossy(&result.stderr)
                    );
                }
            }
            Err(e) => {
                println!(
                    "cargo:warning=Failed to run nvcc for {}: {}",
                    kernel_name, e
                );
            }
        }
    }
}

#[cfg(feature = "cuda")]
fn find_cuda_installation() -> Option<PathBuf> {
    // Try common CUDA installation paths
    let cuda_paths = [
        "/usr/local/cuda",
        "/opt/cuda",
        "/usr/cuda",
        &env::var("CUDA_PATH").unwrap_or_default(),
        &env::var("CUDA_HOME").unwrap_or_default(),
    ];

    for path_str in &cuda_paths {
        if path_str.is_empty() {
            continue;
        }

        let path = PathBuf::from(path_str);
        let nvcc_path = path.join("bin").join("nvcc");

        if nvcc_path.exists() {
            return Some(path);
        }
    }

    // Try to find nvcc in PATH
    if let Ok(output) = Command::new("which").arg("nvcc").output() {
        if output.status.success() {
            let nvcc_path_str = String::from_utf8_lossy(&output.stdout);
            let nvcc_path = nvcc_path_str.trim();
            if let Some(parent) = PathBuf::from(nvcc_path).parent() {
                if let Some(cuda_root) = parent.parent() {
                    return Some(cuda_root.to_path_buf());
                }
            }
        }
    }

    None
}

#[cfg(feature = "rocm")]
fn setup_rocm_compilation() {
    println!("cargo:rustc-link-lib=hip");
    println!("cargo:rustc-link-lib=hiprtc");

    // Find ROCm installation
    let rocm_paths = [
        "/opt/rocm",
        "/usr/rocm",
        &env::var("ROCM_PATH").unwrap_or_default(),
        &env::var("HIP_PATH").unwrap_or_default(),
    ];

    for path_str in &rocm_paths {
        if path_str.is_empty() {
            continue;
        }

        let path = PathBuf::from(path_str);
        let lib_path = path.join("lib");

        if lib_path.exists() {
            println!("cargo:rustc-link-search=native={}", lib_path.display());
            println!("Found ROCm installation at {}", path.display());
            break;
        }
    }
}

fn generate_version_info() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR environment variable should be set by Cargo");
    let dest_path = PathBuf::from(&out_dir).join("version_info.rs");

    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION should be set by Cargo");
    let git_hash = get_git_hash().unwrap_or_else(|| "unknown".to_string());
    let build_time = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();

    let version_info = format!(
        r#"
// Auto-generated version information
pub const BUILD_VERSION: &str = "{version}";
pub const BUILD_GIT_HASH: &str = "{git_hash}";
pub const BUILD_TIME: &str = "{build_time}";
pub const BUILD_FEATURES: &[&str] = &[
    #[cfg(feature = "cuda")]
    "cuda",
    #[cfg(feature = "rocm")]  
    "rocm",
    #[cfg(feature = "gpu")]
    "gpu",
];
"#
    );

    std::fs::write(&dest_path, version_info).expect("Failed to write version info");

    println!("cargo:rustc-env=VERSION_INFO_PATH={}", dest_path.display());
}

fn get_git_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}
