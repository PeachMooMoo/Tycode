# 003: Replace VSCode Title Bar with Usage Statistics

<!-- IMPORTANT: Only document observed behavior and explicit user requirements. 
     Do not speculate about implementation details, file locations, or technical approaches
     unless you have direct evidence from the codebase or user. -->

## Summary
Replace the redundant title bar in the VSCode extension with useful usage statistics and metrics.

## Priority
P2 - Improves user experience by providing valuable usage information instead of redundant UI elements

## Investigation Required
**CRITICAL: Before proposing any solution, the implementing agent MUST:**
- [ ] Search codebase for existing title bar implementation
- [ ] Check how current title bar is rendered and updated
- [ ] Verify where token/cost tracking data is available
- [ ] Understand the current webview architecture
- [ ] Review how data flows from core to VSCode extension

## Current Behavior
The VSCode extension currently displays:
- A title bar below the tab that shows a truncated title (similar to what's already in the tab)
- A trash can icon that is "not at all useful"
These elements provide no value to the user.

## Expected Behavior
User wants the title bar area to display useful statistics instead:
- Summary of context usage
- Sum of input tokens
- Sum of output tokens  
- Sum of money spent
- Sum of AI processing time
- "Anything useful like that"

## Steps to Reproduce (for bugs)
N/A - This is an improvement request

## Technical Context
Only include facts you can observe:
- VSCode extension has a title bar area below tabs
- Currently displays truncated title and trash can icon
- User reports these elements are not useful

## Testing Criteria
- [ ] Verify statistics display correctly
- [ ] Verify numbers update in real-time as conversation progresses
- [ ] Verify calculations are accurate
- [ ] Test with different AI providers if applicable
- [ ] Ensure UI remains responsive with frequent updates

## Acceptance Criteria
- [ ] Title bar shows context usage statistics
- [ ] Shows input and output token counts
- [ ] Shows cumulative cost
- [ ] Shows AI processing time
- [ ] Redundant title and trash can removed
- [ ] Statistics update live during conversation
- [ ] Code follows project style mandates

## Additional Notes
User emphasized wanting practical, useful information displayed instead of redundant UI elements. The specific metrics mentioned were examples - other useful statistics could be included.

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