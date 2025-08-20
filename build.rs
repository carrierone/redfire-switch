use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    
    // Only build assembly on x86_64 targets
    if target_arch == "x86_64" && (target_os == "linux" || target_os == "macos" || target_os == "windows") {
        build_g729_assembly();
    }
    
    println!("cargo:rerun-if-changed=src/g729_asm/");
}

fn build_g729_assembly() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let asm_src_dir = "src/g729_asm";
    
    // Assembly source files
    let asm_files = [
        "autocorrelation.s",
        "levinson_durbin.s", 
        "lsp_quantization.s",
    ];
    
    let mut object_files = Vec::new();
    
    for asm_file in &asm_files {
        let src_path = format!("{}/{}", asm_src_dir, asm_file);
        let obj_name = format!("{}.o", asm_file.trim_end_matches(".s"));
        let obj_path = format!("{}/{}", out_dir, obj_name);
        
        println!("cargo:rerun-if-changed={}", src_path);
        
        // Check if source file exists
        if !Path::new(&src_path).exists() {
            println!("cargo:warning=Assembly source file not found: {}", src_path);
            continue;
        }
        
        // Assemble using GNU assembler (gas)
        let output = Command::new("as")
            .args(&[
                "--64",           // 64-bit mode
                "-o", &obj_path,  // Output object file
                &src_path         // Input assembly file
            ])
            .output();
            
        match output {
            Ok(result) => {
                if !result.status.success() {
                    panic!("Failed to assemble {}: {}", 
                           src_path, 
                           String::from_utf8_lossy(&result.stderr));
                }
                println!("Successfully assembled: {} -> {}", src_path, obj_path);
                object_files.push(obj_path);
            }
            Err(e) => {
                println!("cargo:warning=Failed to run assembler for {}: {}. Trying fallback.", src_path, e);
                
                // Try using clang as fallback assembler
                let clang_output = Command::new("clang")
                    .args(&[
                        "-c",         // Compile only
                        "-x", "assembler", // Treat as assembly
                        "-o", &obj_path,
                        &src_path
                    ])
                    .output();
                    
                match clang_output {
                    Ok(clang_result) => {
                        if !clang_result.status.success() {
                            println!("cargo:warning=Failed to assemble {} with clang: {}", 
                                   src_path,
                                   String::from_utf8_lossy(&clang_result.stderr));
                            continue;
                        }
                        println!("Successfully assembled with clang: {} -> {}", src_path, obj_path);
                        object_files.push(obj_path);
                    }
                    Err(clang_e) => {
                        println!("cargo:warning=Both 'as' and 'clang' failed for {}: as={}, clang={}", 
                               src_path, e, clang_e);
                        continue;
                    }
                }
            }
        }
    }
    
    // Create static library from object files if we have any
    if !object_files.is_empty() {
        let lib_path = format!("{}/libg729_asm.a", out_dir);
        
        let mut ar_cmd = Command::new("ar");
        ar_cmd.args(&["crus", &lib_path]);
        ar_cmd.args(&object_files);
        
        let ar_output = ar_cmd.output();
        match ar_output {
            Ok(result) => {
                if !result.status.success() {
                    panic!("Failed to create static library: {}", 
                           String::from_utf8_lossy(&result.stderr));
                }
                println!("Successfully created static library: {}", lib_path);
                
                // Tell Cargo to link the static library
                println!("cargo:rustc-link-search=native={}", out_dir);
                println!("cargo:rustc-link-lib=static=g729_asm");
                
                // Enable assembly feature if we successfully built
                println!("cargo:rustc-cfg=feature=\"g729_asm\"");
            }
            Err(e) => {
                println!("cargo:warning=Failed to create static library: {}", e);
            }
        }
    } else {
        println!("cargo:warning=No assembly object files were created. Assembly optimization will be disabled.");
    }
}