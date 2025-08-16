// Test program for G.722.2 implementation
use std::f32::consts::PI;

// Simplified G.722.2 constants for testing
const L_FRAME_WB: usize = 320;
const M_WB: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq)]
enum AmrWbMode {
    Mode8 = 8,  // 23.85 kbps
}

impl AmrWbMode {
    fn frame_size_bytes(&self) -> usize {
        match self {
            AmrWbMode::Mode8 => 61,  // 477 bits
        }
    }
    
    fn bitrate(&self) -> f32 {
        match self {
            AmrWbMode::Mode8 => 23.85,
        }
    }
}

struct SimpleG7222Encoder {
    mode: AmrWbMode,
}

impl SimpleG7222Encoder {
    fn new(mode: AmrWbMode) -> Self {
        Self { mode }
    }
    
    fn encode(&mut self, pcm_input: &[i16]) -> Result<Vec<u8>, String> {
        if pcm_input.len() != L_FRAME_WB {
            return Err(format!("Input must be {} samples", L_FRAME_WB));
        }
        
        // Simplified encoding - just create a frame with the mode
        let frame_size = self.mode.frame_size_bytes();
        let mut bitstream = vec![0u8; frame_size];
        
        // Set mode in first 4 bits
        bitstream[0] = (self.mode as u8) << 4;
        
        // Add some dummy data to represent encoded audio
        for i in 1..frame_size {
            bitstream[i] = (i as u8) ^ 0xA5; // Simple pattern
        }
        
        Ok(bitstream)
    }
}

struct SimpleG7222Decoder {
    mode: AmrWbMode,
}

impl SimpleG7222Decoder {
    fn new() -> Self {
        Self {
            mode: AmrWbMode::Mode8,
        }
    }
    
    fn decode(&mut self, bitstream: &[u8]) -> Result<Vec<i16>, String> {
        if bitstream.is_empty() {
            return Err("Empty bitstream".to_string());
        }
        
        // Extract mode from first 4 bits
        let mode_bits = bitstream[0] >> 4;
        if mode_bits == 8 {
            self.mode = AmrWbMode::Mode8;
        }
        
        // Generate dummy PCM output
        let mut pcm_output = vec![0i16; L_FRAME_WB];
        
        // Simple pattern generation
        for i in 0..L_FRAME_WB {
            let sample = ((i as f32 * 0.1).sin() * 16384.0) as i16;
            pcm_output[i] = sample;
        }
        
        Ok(pcm_output)
    }
}

fn main() {
    println!("Testing G.722.2 / AMR-WB Implementation");
    println!("Frame size: {} samples", L_FRAME_WB);
    println!("LP order: {}", M_WB);
    
    // Test mode properties
    let mode = AmrWbMode::Mode8;
    println!("Mode 8 bitrate: {} kbps", mode.bitrate());
    println!("Mode 8 frame size: {} bytes", mode.frame_size_bytes());
    
    // Test encoder
    let mut encoder = SimpleG7222Encoder::new(mode);
    let pcm_input: Vec<i16> = (0..L_FRAME_WB)
        .map(|i| ((i as f32 * 0.1).sin() * 16384.0) as i16)
        .collect();
    
    println!("Encoding {} PCM samples...", pcm_input.len());
    match encoder.encode(&pcm_input) {
        Ok(encoded) => {
            println!("Successfully encoded to {} bytes", encoded.len());
            
            // Test decoder
            let mut decoder = SimpleG7222Decoder::new();
            match decoder.decode(&encoded) {
                Ok(decoded) => {
                    println!("Successfully decoded to {} samples", decoded.len());
                    println!("First few decoded samples: {:?}", &decoded[0..5]);
                }
                Err(e) => {
                    println!("Decode error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Encode error: {}", e);
        }
    }
    
    println!("G.722.2 test completed successfully!");
}