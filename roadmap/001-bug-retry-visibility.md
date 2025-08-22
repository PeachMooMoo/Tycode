# 001: Missing Retry Status Display in VSCode Plugin

<!-- IMPORTANT: Only document observed behavior and explicit user requirements. 
     Do not speculate about implementation details, file locations, or technical approaches
     unless you have direct evidence from the codebase or user. -->

## Summary
Retry attempts are not visible in the VSCode plugin UI when requests fail and are being retried. Users have no indication that a retry is happening.

## Priority
P2 - Users cannot tell if the system is retrying or has failed, leading to confusion about system state

## Investigation Required
**CRITICAL: Before proposing any solution, the implementing agent MUST:**
- [ ] Search for existing retry/error display code in tycode-vscode/src/
- [ ] Check if retry events are being sent from tycode-core but not displayed
- [ ] Verify if this is a missing feature or broken existing feature
- [ ] Review event handling between core and VSCode extension
- [ ] Check subprocessBridge.ts and mainProvider.ts for retry event handling
- [ ] Search for any existing error/retry UI components in webview files

## Current Behavior
When requests fail and are retried, there is no visual indication in the VSCode conversation view that:
- A request has failed
- The system is retrying
- Which retry attempt is currently happening
- What error caused the retry

## Expected Behavior
User wants a UI element in the conversation that shows:
- Text like: "[Request failed - retrying (attempt 1)]" 
- This should appear inline in the conversation when errors occur
- The text should be expandable to show the most recent error details
- Updates as retry attempts progress (attempt 1, 2, 3, etc.)

## Steps to Reproduce (for bugs)
1. Trigger a request that causes a retry (e.g., network timeout)
2. Observe the VSCode conversation view
3. No retry status or error information is displayed to the user

## Technical Context
Only include facts you can observe:
- Retry logic exists in the system (based on improvement-retry-strategy.md)
- VSCode plugin has a conversation view that displays messages
- No current UI elements show retry status

## Testing Criteria
- [ ] Verify retry status appears when requests fail and retry
- [ ] Verify attempt counter increments correctly
- [ ] Verify error details are viewable when expanded
- [ ] Test with different error types that trigger retries
- [ ] Ensure UI updates are real-time

## Acceptance Criteria
- [ ] Retry status displays inline in conversation
- [ ] Shows current attempt number
- [ ] Error details expandable/collapsible
- [ ] Updates live as retries progress
- [ ] Clear indication when retries are exhausted
- [ ] Code follows project style mandates

## Additional Notes
User specifically requested the format "[Request failed - retrying (attempt X)]" with expandable error details.

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