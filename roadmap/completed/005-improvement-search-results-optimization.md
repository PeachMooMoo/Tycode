# Search Results Optimization - Prevent Context Overflow

## Summary
Search operations occasionally return too many matches causing models to run out of context. Need to bound search results and optimize the JSON structure for more compact representation.

## Priority
P1 - High impact on usability when searching common patterns in large codebases

## Investigation Required
**CRITICAL: Before proposing any solution, the implementing agent MUST:**
- [x] Locate the search_files tool implementation
- [x] Analyze current JSON structure for search results
- [x] Identify where before_context and after_context are added
- [x] Check if any existing limits are in place
- [x] Review how search results are consumed by models
- [x] Understand memory/context implications of current format

## Current Behavior
- Broad searches with many matches return all results
- Results include before_context and after_context for each match
- No apparent limit on total results returned
- Large result sets instantly cause models to run out of context
- Current JSON structure may be verbose/redundant

## Expected Behavior
- Search results should be bounded to prevent context overflow
- JSON structure should be more compact (remove before/after context)
- Optional parameter to specify desired context amount
- Model should be able to handle searches that match many files

## Technical Context
User-reported observations:
- "occasionally u are doing a broad search that has a ton of matches"
- "too long and instantly causes the model to run out of context"
- Current results include "before context and after context"
- Need to make "json struct more compact"

## Testing Criteria
- [x] Search with broad pattern doesn't cause context overflow
- [x] Results are bounded appropriately
- [x] JSON structure is more compact
- [x] Optional context parameter works correctly
- [x] Search functionality still provides useful results
- [x] No regression in search accuracy

## Acceptance Criteria
- [x] Maximum result limit implemented
- [x] Before/after context removed or made optional
- [x] Optional parameter for context specification added
- [x] Search results remain useful despite compaction
- [x] Models can handle broad searches without context issues
- [x] Code follows project style mandates

## Additional Notes
Consider:
- Default reasonable limit for search results
- Whether to return count of total matches even if limited
- How to indicate results were truncated
- Balance between context preservation and compactness

---

## Progress Tracking

### Status: Completed
- [x] Investigation completed
- [x] Implementation started
- [x] Testing completed
- [x] Ready for review

### Updates
- **2024-12-20**: Verified implementation is complete. All requested features implemented:
  - `max_results` parameter added (default: 100)
  - `include_context` parameter added (default: false)
  - `context_lines` parameter added (default: 2)
  - JSON structure optimized - context fields only included when present
  - Proper truncation messages when results limited