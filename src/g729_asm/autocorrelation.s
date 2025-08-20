# G.729 Autocorrelation - x86-64 Assembly
# High-performance autocorrelation computation using AVX/SSE

.intel_syntax noprefix
.text

# Constants
.equ L_WINDOW, 240
.equ AUTOCORR_ORDER, 11

# void autocorrelation_avx(const float* windowed_speech, float* r)
.globl autocorrelation_avx
.type autocorrelation_avx, @function
autocorrelation_avx:
    push rbp
    mov rbp, rsp
    push rbx
    push r12
    push r13
    push r14
    push r15
    
    # Parameters:
    # rdi = windowed_speech (const float*)
    # rsi = r (float* - output array[11])
    
    # Clear output array r[0..10]
    xor eax, eax
    mov rcx, AUTOCORR_ORDER
clear_loop:
    mov [rsi + rax*4], dword ptr 0
    inc rax
    cmp rax, rcx
    jl clear_loop
    
    # Main autocorrelation computation
    xor r8, r8                  # k = 0
outer_loop:
    cmp r8, AUTOCORR_ORDER
    jge done
    
    # Initialize accumulator for r[k]
    vxorps ymm0, ymm0, ymm0     # acc = 0.0 (8 floats)
    
    # Calculate limit = L_WINDOW - k
    mov r9, L_WINDOW
    sub r9, r8                  # limit = L_WINDOW - k
    
    # Process 8 samples at a time with AVX
    xor r10, r10                # i = 0
    
inner_loop_avx:
    add r10, 8
    cmp r10, r9
    jg handle_remainder
    
    # Load 8 floats from windowed_speech[i]
    mov r11, r10
    sub r11, 8                  # Adjust for pre-increment
    vmovups ymm1, [rdi + r11*4]
    
    # Load 8 floats from windowed_speech[i + k] 
    mov r12, r11
    add r12, r8                 # i + k
    vmovups ymm2, [rdi + r12*4]
    
    # Multiply and accumulate
    vfmadd231ps ymm0, ymm1, ymm2
    
    jmp inner_loop_avx

handle_remainder:
    # Handle remaining samples (< 8)
    sub r10, 8                  # Back to last processed
    
remainder_loop:
    cmp r10, r9
    jge sum_accumulator
    
    # Load single float and multiply
    vmovss xmm3, [rdi + r10*4]
    mov r12, r10
    add r12, r8
    vmovss xmm4, [rdi + r12*4]
    vmulss xmm3, xmm3, xmm4
    
    # Add to lowest element of accumulator
    vaddss xmm0, xmm0, xmm3
    
    inc r10
    jmp remainder_loop

sum_accumulator:
    # Sum all 8 elements of ymm0 into single float
    vhaddps ymm0, ymm0, ymm0    # Horizontal add: [0+1, 2+3, 4+5, 6+7, 0+1, 2+3, 4+5, 6+7]
    vhaddps ymm0, ymm0, ymm0    # [0+1+2+3, 4+5+6+7, 0+1+2+3, 4+5+6+7, ...]
    
    # Extract upper 128 bits and add to lower 128 bits
    vextractf128 xmm1, ymm0, 1
    vaddss xmm0, xmm0, xmm1
    
    # Store result in r[k]
    vmovss [rsi + r8*4], xmm0
    
    inc r8
    jmp outer_loop

done:
    # Clean up AVX state
    vzeroupper
    
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret

# void autocorrelation_sse(const float* windowed_speech, float* r)
.globl autocorrelation_sse
.type autocorrelation_sse, @function
autocorrelation_sse:
    push rbp
    mov rbp, rsp
    push rbx
    push r12
    push r13
    
    # Parameters:
    # rdi = windowed_speech
    # rsi = r
    
    # Clear output array
    xor eax, eax
    mov rcx, AUTOCORR_ORDER
clear_sse_loop:
    mov [rsi + rax*4], dword ptr 0
    inc rax
    cmp rax, rcx
    jl clear_sse_loop
    
    # Main computation with SSE (4 floats at a time)
    xor r8, r8
outer_sse_loop:
    cmp r8, AUTOCORR_ORDER
    jge sse_done
    
    xorps xmm0, xmm0            # acc = 0.0 (4 floats)
    mov r9, L_WINDOW
    sub r9, r8                  # limit
    
    xor r10, r10
inner_sse_loop:
    add r10, 4
    cmp r10, r9
    jg sse_remainder
    
    mov r11, r10
    sub r11, 4
    movups xmm1, [rdi + r11*4]
    
    mov r12, r11
    add r12, r8
    movups xmm2, [rdi + r12*4]
    
    mulps xmm1, xmm2
    addps xmm0, xmm1
    
    jmp inner_sse_loop

sse_remainder:
    sub r10, 4
sse_remainder_loop:
    cmp r10, r9
    jge sse_sum
    
    movss xmm3, [rdi + r10*4]
    mov r12, r10
    add r12, r8
    movss xmm4, [rdi + r12*4]
    mulss xmm3, xmm4
    addss xmm0, xmm3
    
    inc r10
    jmp sse_remainder_loop

sse_sum:
    # Sum 4 elements of xmm0
    haddps xmm0, xmm0           # [0+1, 2+3, 0+1, 2+3]
    haddps xmm0, xmm0           # [0+1+2+3, 0+1+2+3, ...]
    
    movss [rsi + r8*4], xmm0
    
    inc r8
    jmp outer_sse_loop

sse_done:
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret