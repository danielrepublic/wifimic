# Learnings — wifimic-lan-audio

Conventions, patterns, and successful approaches discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## 2026-08-22T07:08:01.8236967Z

- The `.codegraph` Windows junction is correctly ignored by the root `/.codegraph/` rule, while `.omo/` remains trackable.
- `gh repo create wifimic --private --source=. --remote=origin` created the private remote but did not push the existing `main` branch; the explicit `git push -u origin main` was required.
- Repeating the create command failed non-destructively with `GraphQL: Name already exists on this account (createRepository)` and left `origin` and the private repository intact.

## 2026-08-22T07:19:29.6980498Z

- CodeGraph returned no relevant indexed workspace/reference symbols for this repository, so the required micdriver and Rust guidance files were inspected directly.
- The scaffold follows micdriver's resolver 2, edition 2021, Rust 1.97.1, rustfmt, and workspace Clippy `all = "deny"` conventions while intentionally leaving all four members dependency-free.
- The failure probe must copy the four member directories alongside the copied root manifest to reach the deliberately missing `crates/does_not_exist` member; the tracked root manifest hash remained unchanged.
