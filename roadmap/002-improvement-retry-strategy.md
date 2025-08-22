# 002: Implement Intelligent Retry Strategy for Transient Errors

<!-- IMPORTANT: Only document observed behavior and explicit user requirements. 
     Do not speculate about implementation details, file locations, or technical approaches
     unless you have direct evidence from the codebase or user. -->

## Summary
Current retry logic is binary - either retry forever or fail immediately. Need a middle ground for transient errors like network timeouts that should retry with backoff but eventually give up.

## Priority
P1 - Network errors currently cause immediate failure, significantly impacting reliability

## Investigation Required
**CRITICAL: Before proposing any solution, the implementing agent MUST:**
- [ ] Search codebase for existing retry logic implementation
- [ ] Check how current retry decisions are made
- [ ] Verify which errors currently retry forever vs fail immediately
- [ ] Understand the current error handling architecture
- [ ] Review how errors are classified and handled

## Current Behavior
- Throttle errors: Retry forever
- Context window exceeded: Fail immediately
- Network timeouts: Fail immediately (but should retry)
- No exponential backoff for retries
- No maximum retry limit for transient errors

Example error that should be retried:
```
Error: Terminal error: Unknown error from bedrock: Unhandled(Unhandled { 
  source: DispatchFailure(DispatchFailure { 
    source: ConnectorError { 
      kind: Timeout, 
      source: hyper_util::client::legacy::Error(Connect, HttpTimeoutError { 
        kind: "HTTP connect", 
        duration: 3.1s 
      }), 
      connection: Unknown 
    } 
  }), 
  meta: ErrorMetadata { code: None, message: None, extras: None } 
})
```

## Expected Behavior
Three-tier retry strategy:
1. **No Retry**: Terminal errors (context window exceeded, invalid API key)
2. **Limited Retry**: Transient errors (network timeout, connection errors) - retry 3-5 times with exponential backoff
3. **Unlimited Retry**: Rate limiting/throttling - retry forever with backoff

## Technical Context
The AI provider implementations handle errors but currently have binary retry logic. Network timeouts like the example error are treated as terminal failures when they should be retried.

## Testing Criteria
- [ ] Unit test for error classification logic
- [ ] Unit test for exponential backoff calculation
- [ ] Integration test simulating network timeout and verifying retry
- [ ] Test that terminal errors fail immediately
- [ ] Test that rate limit errors retry indefinitely
- [ ] Verify retry attempts are logged for debugging

## Acceptance Criteria
- [ ] Network timeouts retry 3-5 times before failing
- [ ] Exponential backoff implemented (e.g., 1s, 2s, 4s, 8s)
- [ ] Terminal errors fail immediately without retry
- [ ] Rate limiting continues to retry forever
- [ ] Clear logging of retry attempts and final failure
- [ ] No regression in existing error handling
- [ ] Code follows project style mandates

## Additional Notes
Consider making retry configuration adjustable via settings. Initial implementation should focus on classifying these error types correctly:
- Network/connection timeouts -> Transient
- HTTP 429/503 -> RateLimit  
- Context window/invalid request -> Terminal
- HTTP 500/502 -> Transient (service errors often temporary)

---

## Progress Tracking
<!-- Update this section as work progresses. DO NOT delete previous updates, add new ones. -->

### Status: Not Started
- [ ] Investigation completed
- [ ] Implementation started
- [ ] Testing completed
- [ ] Ready for review

### Updates
<!-- Add dated entries as progress is made -->