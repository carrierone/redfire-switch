# μ-Law Codec Conversion Systematic Issues - Help Wanted

**Labels:** `bug`, `help-wanted`, `audio-codecs`, `codec-processing`

## Summary
The μ-Law (G.711 μ-law) codec implementation has systematic issues with round-trip conversion, where many values fail to convert back to their original form within acceptable tolerances.

## Current Status
- Tests are marked as `#[ignore]` in:
  - `src/codec_optimized.rs:262` (`test_codec_processor_tables`)
  - `src/cesopsn_ni2_integration.rs:588` (`test_ulaw_conversion`)
- μ-Law conversion works but round-trip accuracy is problematic

## Technical Details

**Failing Tests:**
- `codec_optimized::tests::test_codec_processor_tables`
- `cesopsn_ni2_integration::tests::test_ulaw_conversion`

**Example Failure:**
```
μ-Law round-trip failed for 0: 0 -> -128 -> 218 (diff: 218, tolerance: 255)
```

**Code Locations:**
- `src/codec_optimized.rs:123-141` (μ-Law to linear conversion)
- `src/codec_optimized.rs:164-194` (linear to μ-Law conversion)
- `src/codec_optimized.rs:262-292` (round-trip test)

## Investigation Needed

1. **Algorithm Verification:**
   - Review ITU-T G.711 specification compliance
   - Verify μ-Law encoding/decoding tables are correct
   - Check if implementation matches standard reference

2. **Quantization Analysis:**
   - Understand expected quantization errors for μ-Law
   - Determine appropriate tolerance levels for lossy compression
   - Research industry-standard test expectations

3. **Implementation Review:**
   - Check lookup table generation in `init_tables()`
   - Verify bit manipulation in conversion functions
   - Review overflow protection (saturating arithmetic)

## Current Implementation

**μ-Law to Linear (simplified):**
```rust
fn ulaw_to_linear_slow(&self, ulaw: u8) -> i16 {
    let ulaw = !ulaw;
    let sign = if (ulaw & 0x80) != 0 { -1 } else { 1 };
    let exponent = (ulaw >> 4) & 0x07;
    let mantissa = ulaw & 0x0F;
    
    let magnitude = if exponent == 0 {
        (mantissa << 2) + 33
    } else {
        let shift_amount = (exponent as i32).min(10);
        ((mantissa << 2) + 33) << shift_amount
    };
    
    let result = (sign as i32).saturating_mul(magnitude as i32);
    result.clamp(-32768, 32767) as i16
}
```

## Issues to Investigate

1. **Zero Value Handling:**
   - μ-Law value 0 maps to linear -128, then back to μ-Law 218
   - This suggests a bias or offset issue

2. **Symmetry Problems:**
   - Positive and negative values may have different error characteristics
   - Quantization may not be symmetric around zero

3. **Reference Implementation:**
   - Compare with ITU-T reference implementation
   - Verify against other established μ-Law libraries

## Proposed Solutions

**Option 1: Algorithm Correction**
- Fix the encoding/decoding algorithms to match G.711 exactly
- Ensure proper handling of special cases (zero, min/max values)

**Option 2: Tolerance Adjustment**
- Research appropriate test tolerances for μ-Law
- Update test expectations to match lossy compression reality

**Option 3: Reference Implementation**
- Use proven reference implementation or lookup tables
- Validate against known-good test vectors

## Files to Review
- `src/codec_optimized.rs` (main codec implementation)
- ITU-T G.711 specification
- Reference implementations in other projects

## Skills Needed
- Digital signal processing knowledge
- μ-Law/A-Law codec algorithms
- ITU-T telecommunications standards
- Audio codec testing methodologies

## Expected Outcome
- μ-Law conversion passes round-trip tests with appropriate tolerances
- Implementation fully complies with G.711 standard
- Clear documentation of expected quantization behavior

## Test Data Needed
- ITU-T G.711 reference test vectors
- Industry-standard tolerance specifications
- Comparison with established codec libraries

## Related Issues
- May affect A-Law conversion as well
- Critical for telephony audio quality
- Impacts CESoPSN and TDM circuit emulation