# Learnings — v0-2-0-update-mechanism

Conventions, patterns, and successful approaches discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

- Todo 1: Renamed only the server CLI surface (`Command::CheckUpdate` to `Command::Update`, parser verb `check-update` to `update`, and the `run_main` arm), updated the README literal, and retained all `update_cli` `check_update_*` identifiers; the parser now has an explicit legacy-verb rejection test.

- Todo 5 (Final): Retired `deploy/windows/update-wifimic-client.ps1` via `git rm`. Rewrote three doc sections to replace references to the deleted script with `wifimic_client_updater.exe` documentation:
  - `docs/release-process.md` §4 Windows bullet: now describes double-click execution, no `-Tag`, latest-only nature, and the one-time bootstrap-reinstall requirement for pre-v0.2.0 machines. §4 reference list: replaced script path with updater binary path.
  - `docs/deployment.md` §1 table: "Windows 更新腳本" row → "Windows 更新工具" referencing `wifimic_client_updater.exe` in install directory.
  - `docs/deployment.md` §7 (entire §7.1–§7.6): completely rewritten for the updater binary — prerequisites, double-click flow, UAC elevation + decline behavior, internal update flow with `UpdaterOutcome` variants, automatic rollback contract, and post-update verification. Quoted verbatim console strings from the shipped binary (`檢查中...`, `已是最新版本`, `發現新版本，更新中...`, `已更新至 {tag}`, failure messages).
  - `docs/deployment.md` §12 reference list: replaced script reference with updater binary.
  - `docs/adr/0001-...md`: reworded two lines to avoid grep-matching the deleted script filename while preserving historical meaning.
  - Repo-wide grep sweep: zero matches for `update-wifimic-client.ps1` across `.md`, `.yml`, `.ps1`, `.rs` files (excluding `.omo`/`.git`/`target`).
