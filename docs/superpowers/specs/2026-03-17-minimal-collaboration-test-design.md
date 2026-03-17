# Minimal Collaboration Test Suite Design

**Date:** 2026-03-17  
**Purpose:** Replace complex 27-test suite with single essential collaboration test  
**Goal:** Validate SpacetimeDB real-time collaborative functionality with maximum CI efficiency

## Problem Statement

Current test suite has 27 tests across 5 files taking 25-30 minutes to run, causing CI timeouts. Only need to validate core collaborative functionality: checkbox state syncs in real-time between multiple browser sessions.

## Solution: Single Collaboration Test

### Architecture
- **Single test file:** `collaboration-test.spec.js`
- **Multi-browser context:** Two browser sessions in one test
- **Real-time validation:** Click in one browser, verify in another
- **Runtime target:** 1-2 minutes vs 25-30 minutes

### Test File Structure
```
/checkboxes-clean/
├── collaboration-test.spec.js  # Single essential test
└── [DELETE ALL existing .spec.js files]
```

Files to delete:
- `smoke-test.spec.js`
- `test.spec.js` 
- `test-mouse.spec.js`
- `test-panning.spec.js`
- `test-fixed.spec.js`

### Test Implementation

#### Test Name
`"real-time collaborative checkbox synchronization"`

#### Test Flow
1. **Setup Phase**
   - Create two browser contexts (`browser1`, `browser2`)
   - Navigate both to the application URL
   - Wait for WASM initialization in both browsers
   - Wait for SpacetimeDB connection in both browsers

2. **Action Phase**
   - Identify the first visible checkbox in both browsers
   - Record initial state (should be unchecked)
   - Click the checkbox in `browser1`

3. **Verification Phase**
   - Wait for real-time synchronization
   - Verify the same checkbox is checked in `browser2`
   - Ensure state consistency between browsers

4. **Cleanup Phase**
   - Close both browser contexts
   - Reset SpacetimeDB state if needed

#### Error Handling
- **SpacetimeDB connection timeouts:** 10-second timeout with clear error messages
- **WebSocket failures:** Retry logic and graceful degradation
- **State sync delays:** Appropriate wait conditions for real-time updates
- **WASM loading issues:** Wait for app initialization signals

#### Success Criteria
- Both browsers successfully connect to SpacetimeDB
- Checkbox click in browser1 triggers state change
- State change appears in browser2 within reasonable time (~2-3 seconds)
- No JavaScript errors or connection failures

### Technical Implementation Details

#### Browser Context Setup
```javascript
// Create isolated browser contexts
const context1 = await browser.newContext();
const context2 = await browser.newContext();
const page1 = await context1.newPage();
const page2 = await context2.newPage();
```

#### SpacetimeDB Connection Validation
```javascript
// Wait for SpacetimeDB connection in both browsers
await page1.waitForFunction(() => window.spacetimedbConnected);
await page2.waitForFunction(() => window.spacetimedbConnected);
```

#### Checkbox Targeting Strategy
- Target first visible checkbox to avoid coordinate dependencies
- Use consistent selector strategy across both browsers
- Verify checkbox exists and is interactable before clicking

#### State Synchronization Verification
- Poll for state change in browser2 after browser1 action
- Use Playwright's `waitForFunction` for reliable state checking
- Include timeout handling for sync failures

### Performance Benefits
- **Runtime reduction:** 25-30 minutes → 1-2 minutes (~95% reduction)
- **CI reliability:** No more timeout issues
- **Focused testing:** Only validates core collaborative value
- **Maintenance simplicity:** Single test to maintain vs 27 tests

### Risk Mitigation
- **Coverage reduction:** Accept reduced test coverage for CI stability
- **Core functionality focus:** Single test validates the main value proposition
- **Manual testing backup:** Complex edge cases can be tested manually
- **Incremental expansion:** Can add more tests later if needed

## Implementation Plan

1. **Create new test file:** `collaboration-test.spec.js`
2. **Delete existing test files:** All 5 current `.spec.js` files
3. **Update CI configuration:** Reduce timeout expectations
4. **Test and validate:** Ensure new test passes reliably
5. **Document changes:** Update testing documentation

## Acceptance Criteria

- ✅ Single test file replaces all existing tests
- ✅ Test validates real-time collaborative functionality  
- ✅ Runtime under 3 minutes in CI
- ✅ No more CI timeout issues
- ✅ Clear error messages for debugging
- ✅ Reliable execution across CI environments