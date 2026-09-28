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

## Refutation — in rounds, with a limit

Every change (feature, fix, refactor, visual change) goes through the **`refuter`** subagent (`.claude/agents/refuter.md`) before it is called done. No exception for "small" ones.

1. Finish the change, with tests green, fmt and clippy passing.
2. **Round 1:** run the `refuter`, telling it the round, the commits and the surfaces touched (VPN engine, settings, tray, updater, UI, scripts, site).
3. Fix every blocker and must-fix. Fix the should-fix and nice-to-have items too, in the same change; they do not trigger a new round.
4. **Round 2:** run the `refuter` again **only on the fixes**. It checks each must-fix is gone and looks for regressions in what was touched.
5. Repeat until zero blocker/must-fix, **at most 3 rounds**. Whatever is still open after round 3 is reported to the human, not hidden.

<!-- SEMBLE_START -->
For CLI fallback or sub-agents without MCP access, use:

```bash
semble search "authentication flow" ./my-project --max-snippet-lines 10
semble search "deployment guide" ./my-project --content docs
semble search "database host port" ./my-project --content config
semble find-related src/auth.py 42 ./my-project
```
<!-- SEMBLE_END -->
