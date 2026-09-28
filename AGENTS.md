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

Every change (feature, fix, refactor, visual change) goes through the **`refuter`** subagent (`.claude/agents/refuter.md`) before it is called done. No exception for "small" ones. Tokens are limited, so the refuter spends them on **judgment**: whatever is repeated or mechanical comes out of it, the rigor does not (budget rules in its §0).

**Before round 1 (you, not the refuter):**

1. Finish the change. One refuter per delivery, never mid-work.
2. Run the gates and keep each **exit code**: `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, the `scripts/*.test.sh` and `npm test` the change touches and, if dependencies changed, `cargo audit`. All green before calling the refuter: it does not spend tokens finding what a tool finds.
3. Pick the **tier**: **T0** text only (docs, agent files, comments: one round, no frames); **T1** standard; **T2** sensitive (`pkexec`/root, credentials, updater and installer, config written as root, child process lifecycle): spawn it with `model: "opus"`. When in doubt, go up. The refuter can raise the tier, never lower it.
4. If the UI changed, the refuter will capture it at every size (`scripts/screenshots.sh`: the real app on a virtual display, every shot × theme × language × six window sizes from 900×620 to 2560×1440, and `--video` for recordings plus a 12-frame strip the refuter can read; needs `xvfb` and `ffmpeg`, and CI uploads the same set as the `screens` artifact). Nothing opens a window on the desktop of whoever is using the machine: frames and recordings are always offscreen.

**Round 1 briefing** (everything it needs, so it does not go looking):

```
Round 1, tier T1.
Commits: <sha>..<sha> — <what the delivery does, one line>
Files: <list>
Surfaces: <VPN engine, settings, tray, updater, UI, scripts, site; "motion" if an animation or flow changed>
Gates: fmt → 0 · clippy → 0 · test → 0 · scripts/screenshots.test.sh → 0 · audit → not run (Cargo.lock unchanged)
Already verified: <what not to redo>
```

**The loop:**

1. **Round 1:** run the `refuter` with the briefing.
2. Fix every blocker and must-fix, and the should-fix and nice-to-have items too, all at once, in the same change; small ones do not trigger a new round. Rerun the gates, commit.
3. **Round 2: continue the same refuter** (Claude Code: `SendMessage` to the round 1 agent; other tools: the equivalent) with the round, the fix commits and the gates. It already read the rules, the code and the frames, so it reviews only the fix diff. Start a new refuter only if the old one is gone, and then pass it the previous report.
4. Repeat until zero blocker/must-fix, **at most 3 rounds**. Whatever is still open after round 3 is reported to the human, not hidden. A rerun of the sensitive part on the strongest model, when the refuter raised the tier, does not count as a round.

When reporting to the human: how many rounds ran, the tier, what each found and what was fixed, **the tokens of each round** (in the subagent's result) and what was left out and why. Tokens per round show whether the budget works.

<!-- SEMBLE_START -->
For CLI fallback or sub-agents without MCP access, use:

```bash
semble search "authentication flow" ./my-project --max-snippet-lines 10
semble search "deployment guide" ./my-project --content docs
semble search "database host port" ./my-project --content config
semble find-related src/auth.py 42 ./my-project
```
<!-- SEMBLE_END -->
