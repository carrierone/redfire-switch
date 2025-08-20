# DTMF Detector Timing Issues - Help Wanted

**Labels:** `bug`, `help-wanted`, `audio-processing`, `dtmf`

## Summary
The DTMF detector test is experiencing timing issues where detection events are not being emitted within expected timeframes, causing test failures.

## Current Status
- Test is currently marked as `#[ignore]` in `src/dtmf_processor.rs:702`
- The detector requires both tone generation AND silence detection to emit events
- Timeout issues suggest the state machine timing may need adjustment

## Technical Details

**Failing Test:** `dtmf_processor::tests::test_dtmf_detector`

**Error:** 
```
thread 'dtmf_processor::tests::test_dtmf_detector' panicked at src/dtmf_processor.rs:714:86:
called `Result::unwrap()` on an `Err` value: Elapsed(())
```

**Code Location:** `src/dtmf_processor.rs:695-720`

## Investigation Needed

1. **State Machine Analysis:**
   - Review DTMF detection state transitions
   - Verify minimum tone duration requirements (currently 40ms)
   - Check if silence detection timing is appropriate

2. **Event Emission Logic:**
   - Investigate when `DtmfEvent::DigitDetected` events are sent
   - Verify event sender/receiver setup in tests
   - Check if events are being lost in the broadcast channel

3. **Timing Configuration:**
   - Review if 500ms timeout is sufficient for processing
   - Consider if detector needs different timing for test vs. production

## Proposed Solutions

**Option 1: Fix State Machine Timing**
- Adjust detection thresholds and timing windows
- Ensure events are emitted reliably when tone ends

**Option 2: Improve Test Design**
- Use more realistic audio samples with proper frequency content
- Add debugging output to trace state machine transitions
- Consider using mocks for deterministic testing

**Option 3: Configuration Adjustment**
- Add test-specific configuration for faster detection
- Separate test timing from production timing requirements

## Files to Review
- `src/dtmf_processor.rs` (main DTMF detection logic)
- `src/dtmf_processor.rs:695-720` (failing test)
- DTMF detection state machine implementation

## Skills Needed
- Audio signal processing knowledge
- DTMF detection algorithms
- Rust async/await and timing
- Unit testing best practices

## Expected Outcome
- DTMF detector test passes reliably
- State machine timing is well-documented
- Detection logic works correctly in both test and production environments

## Related Issues
- Part of overall test coverage improvement initiative
- May be related to audio codec processing timing