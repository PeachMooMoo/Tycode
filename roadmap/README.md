# Roadmap

This directory contains pending bugs and improvements that have been identified but not yet implemented.

## Structure

Each issue is a separate markdown file named with the pattern:
- Bugs: `bug-{short-description}.md`
- Improvements: `improvement-{short-description}.md`

## Creating New Issues

When describing a bug or improvement casually, it will be expanded into a detailed issue using the template structure found in `ISSUE_TEMPLATE.md`.

## Issue States

Files remain in this directory until:
- Implementation begins (move to in-progress/)
- Issue is resolved (move to completed/)
- Issue is abandoned (move to abandoned/)

## Priority Levels

- P0: Critical - Breaking functionality
- P1: High - Significant impact on user experience
- P2: Medium - Notable improvement opportunity
- P3: Low - Nice to have

## For Implementation Agents

Each issue contains sufficient detail for independent implementation. Look for:
- Clear problem statement
- Expected behavior
- Investigation requirements
- Testing criteria

### Working on an Issue

1. **Investigation First** - Complete the investigation checklist before proposing solutions
2. **Update Progress** - Add dated entries to the Progress Tracking section as you work
3. **Don't Delete History** - Add new updates without removing previous ones
4. **Move When Complete** - Once fully resolved and tested:
   - Create `roadmap/resolved/` directory if it doesn't exist
   - Move the issue file to `roadmap/resolved/`
   - Final update should indicate resolution date and summary