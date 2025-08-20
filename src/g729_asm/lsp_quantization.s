# G.729 LSP Quantization - x86-64 Assembly
# High-performance LSP vector quantization using SIMD

.intel_syntax noprefix
.text

# Quantize LSP vector using SIMD distance computation
# void lsp_quantization_avx(const float* lsp, const float* codebook, 
#                          int codebook_size, int* best_index, float* min_distance)
.globl lsp_quantization_avx
.type lsp_quantization_avx, @function
lsp_quantization_avx:
    push rbp
    mov rbp, rsp
    push rbx
    push r12
    push r13
    push r14
    push r15
    
    # Parameters:
    # rdi = lsp (const float* - LSP vector[10])  
    # rsi = codebook (const float* - codebook entries[codebook_size][10])
    # rdx = codebook_size (int)
    # rcx = best_index (int* - output)
    # r8  = min_distance (float* - output)
    
    # Initialize best results
    mov dword ptr [rcx], 0         # best_index = 0
    mov eax, 0x7F800000            # +infinity
    mov [r8], eax                  # min_distance = +infinity
    movss xmm0, [r8]               # Load current min_distance
    
    # Process codebook entries
    xor r9, r9                     # index = 0
    
codebook_loop:
    cmp r9, rdx                    # Compare with codebook_size
    jge quantization_done
    
    # Compute squared distance using AVX
    # Distance = sum((lsp[i] - codebook[index][i])^2) for i = 0 to 9
    
    # Load LSP vector (10 floats) - need 2 AVX registers (8 + 2)
    vmovups ymm1, [rdi]            # lsp[0..7]
    vmovsd xmm2, [rdi + 32]        # lsp[8..9]
    
    # Calculate codebook entry address: codebook + index * 10 * 4
    mov r10, r9
    imul r10, 40                   # index * 10 * sizeof(float)
    add r10, rsi                   # codebook[index]
    
    # Load codebook entry
    vmovups ymm3, [r10]            # codebook[index][0..7]
    vmovsd xmm4, [r10 + 32]        # codebook[index][8..9]
    
    # Compute differences
    vsubps ymm5, ymm1, ymm3        # diff[0..7] = lsp[0..7] - codebook[index][0..7]
    vsubps xmm6, xmm2, xmm4        # diff[8..9] = lsp[8..9] - codebook[index][8..9]
    
    # Square the differences
    vmulps ymm5, ymm5, ymm5        # diff[0..7]^2
    vmulps xmm6, xmm6, xmm6        # diff[8..9]^2
    
    # Sum all squared differences
    # First sum the 8 elements in ymm5
    vhaddps ymm5, ymm5, ymm5       # Horizontal add: [0+1, 2+3, 4+5, 6+7, 0+1, 2+3, 4+5, 6+7]
    vhaddps ymm5, ymm5, ymm5       # [0+1+2+3, 4+5+6+7, 0+1+2+3, 4+5+6+7, ...]
    
    # Extract upper 128 bits and add to lower
    vextractf128 xmm7, ymm5, 1
    vaddss xmm5, xmm5, xmm7        # Sum of elements 0-7
    
    # Add the remaining 2 elements (8,9)
    vhaddps xmm6, xmm6, xmm6       # Add elements 8+9
    vaddss xmm5, xmm5, xmm6        # Total distance
    
    # Compare with current minimum
    comiss xmm5, xmm0
    jae next_entry                 # If distance >= min_distance, skip
    
    # New minimum found
    movss xmm0, xmm5               # Update min_distance
    mov [rcx], r9d                 # Update best_index
    movss [r8], xmm0               # Store new min_distance

next_entry:
    inc r9
    jmp codebook_loop

quantization_done:
    vzeroupper                     # Clean up AVX state
    
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret

# SSE version for systems without AVX
.globl lsp_quantization_sse
.type lsp_quantization_sse, @function
lsp_quantization_sse:
    push rbp
    mov rbp, rsp
    push rbx
    push r12
    push r13
    push r14
    push r15
    
    # Same parameters as AVX version
    
    # Initialize
    mov dword ptr [rcx], 0
    mov eax, 0x7F800000
    mov [r8], eax
    movss xmm0, [r8]
    
    xor r9, r9

sse_codebook_loop:
    cmp r9, rdx
    jge sse_done
    
    # Load LSP vector using SSE (need 3 loads for 10 floats)
    movups xmm1, [rdi]             # lsp[0..3]
    movups xmm2, [rdi + 16]        # lsp[4..7]
    movsd xmm3, [rdi + 32]         # lsp[8..9]
    
    # Calculate codebook entry address
    mov r10, r9
    imul r10, 40
    add r10, rsi
    
    # Load codebook entry
    movups xmm4, [r10]             # codebook[index][0..3]
    movups xmm5, [r10 + 16]        # codebook[index][4..7]
    movsd xmm6, [r10 + 32]         # codebook[index][8..9]
    
    # Compute differences and square them
    subps xmm1, xmm4
    mulps xmm1, xmm1               # diff[0..3]^2
    
    subps xmm2, xmm5  
    mulps xmm2, xmm2               # diff[4..7]^2
    
    subps xmm3, xmm6
    mulps xmm3, xmm3               # diff[8..9]^2
    
    # Sum all elements
    addps xmm1, xmm2               # sum[0..3] + sum[4..7]
    haddps xmm1, xmm1              # Horizontal add pairs
    haddps xmm1, xmm1              # Final horizontal add
    
    haddps xmm3, xmm3              # Add elements 8+9
    addss xmm1, xmm3               # Total distance
    
    # Compare and update if better
    comiss xmm1, xmm0
    jae sse_next_entry
    
    movss xmm0, xmm1
    mov [rcx], r9d
    movss [r8], xmm0

sse_next_entry:
    inc r9
    jmp sse_codebook_loop

sse_done:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret

# Scalar fallback version
.globl lsp_quantization_scalar
.type lsp_quantization_scalar, @function
lsp_quantization_scalar:
    push rbp
    mov rbp, rsp
    push rbx
    push r12
    push r13
    push r14
    
    # Initialize
    mov dword ptr [rcx], 0
    mov eax, 0x7F800000
    mov [r8], eax
    movss xmm0, [r8]
    
    xor r9, r9

scalar_codebook_loop:
    cmp r9, rdx
    jge scalar_done
    
    # Calculate distance for this codebook entry
    xorps xmm1, xmm1               # distance = 0.0
    xor r10, r10                   # i = 0
    
    # Get codebook entry base address
    mov r11, r9
    imul r11, 40                   # index * 10 * 4
    add r11, rsi
    
scalar_distance_loop:
    cmp r10, 10
    jge scalar_distance_done
    
    # diff = lsp[i] - codebook[index][i]
    movss xmm2, [rdi + r10*4]      # lsp[i]
    movss xmm3, [r11 + r10*4]      # codebook[index][i]
    subss xmm2, xmm3               # diff
    mulss xmm2, xmm2               # diff^2
    addss xmm1, xmm2               # distance += diff^2
    
    inc r10
    jmp scalar_distance_loop

scalar_distance_done:
    # Compare with current minimum
    comiss xmm1, xmm0
    jae scalar_next_entry
    
    movss xmm0, xmm1
    mov [rcx], r9d
    movss [r8], xmm0

scalar_next_entry:
    inc r9
    jmp scalar_codebook_loop

scalar_done:
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret