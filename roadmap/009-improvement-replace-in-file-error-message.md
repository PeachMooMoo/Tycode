# 009 - Improvement: Enhance Error Messages for Failed Search Replace Operations

## Issue
When using find-replace operations via /fileapi findreplace, if the search block doesn't exactly match any text in the file, the current error message is generic and doesn't provide helpful suggestions. This forces the AI model to inspect the entire file content or use additional tools to identify and fix potential typos in the search query.

## Proposed Solution
Implement string similarity-based suggestions for failed search replace operations:
1. When no exact match is found, compute similarity scores (using LCS or character-based metrics) between the search block and all lines/sections in the file
2. Identify the top N closest matches (e.g., N=3)
3. Include suggestions in the error message: "Did you mean: [closest_match_1], [closest_match_2], ..."
4. Optionally, show the context around the suggested matches

## Benefits
- Reduces the number of tool calls needed for file modifications
- Provides immediate feedback to help correct minor typos
- Improves user experience for iterative file editing
- Experiments with AI-assisted error correction

## Acceptance Criteria
- Error message includes "Did you mean: [suggestions]" when exact match fails
- Suggestions are relevant (top similarity matches)
- No performance impact on successful operations
- Integration with existing replace_in_file logic

## Implementation Notes
- Add similarity function (e.g., levenshtein distance) to tool
- Parse file content and search for best matches
- Ensure suggestions don't exceed reasonable limits
- Test with various file types and search patterns
- Consider configurable threshold for "close enough" matches