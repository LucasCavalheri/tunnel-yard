# AGENTS.md

## Semble Code Search

Use `mcp__semble__search` to find where behavior is implemented instead of
using Grep or Glob to discover files. After Semble returns a file and line,
navigate directly to that location and read the relevant function or class.

Use `content="docs"` for documentation, `content="config"` for configuration
and `content="all"` when both code and non-code files are relevant. Use
`mcp__semble__find_related` to discover similar implementations after locating
the primary one. Use Grep only when every occurrence of a literal string is
needed, such as all callers of a renamed function.

## Testing is required for every change

Every feature, fix, refactor, script, configuration, packaging or documentation
change must include or update automated tests, or explicitly demonstrate which
existing tests cover it. This applies to UI, networking, persistence, security,
error handling, installers, updaters, platform adapters, scripts,
documentation-driven behavior and internal refactors with observable effects.
No implementation task is complete without test evidence.

Tests must exercise the behavior, not merely assert that source text contains
expected words. Prefer the smallest realistic test that runs the affected
boundary end to end in an isolated temporary environment. Test the happy path,
relevant invalid input and failure paths, and the regression that motivated the
change. For integration boundaries, simulate the real lifecycle and verify
observable outputs and preserved state (for example: install, upgrade, launch,
rollback and removal), without touching the developer's host installation.

Before considering a bug fixed, add a regression test that fails against the
old behavior and passes with the fix. Run the focused tests and the complete
appropriate test suite. A textual/source-shape assertion alone is not
sufficient evidence for a behavioral fix.

When a platform cannot be exercised locally, keep a platform-specific test or
fixture in the repository and run it in the native CI matrix. Do not silently
replace a real platform test with a weaker string check.

<!-- SEMBLE_START -->
For CLI fallback or sub-agents without MCP access, use:

```bash
semble search "authentication flow" ./my-project --max-snippet-lines 10
semble search "deployment guide" ./my-project --content docs
semble search "database host port" ./my-project --content config
semble find-related src/auth.py 42 ./my-project
```
<!-- SEMBLE_END -->
