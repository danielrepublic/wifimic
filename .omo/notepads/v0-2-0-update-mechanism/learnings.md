# Learnings — v0-2-0-update-mechanism

Conventions, patterns, and successful approaches discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

- Todo 1: Renamed only the server CLI surface (`Command::CheckUpdate` to `Command::Update`, parser verb `check-update` to `update`, and the `run_main` arm), updated the README literal, and retained all `update_cli` `check_update_*` identifiers; the parser now has an explicit legacy-verb rejection test.
