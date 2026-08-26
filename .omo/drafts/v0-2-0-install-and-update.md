---
slug: v0-2-0-install-and-update
status: drafting
intent: clear
review_required: false
pending-action: write .omo/plans/v0-2-0-install-and-update.md
approach: Ship a compiled `wifimic-client-updater.exe` (new Cargo bin in apps/wifimic_client, sharing the `wifimic_update` crate) that mirrors the server's release-archive-download/verify/atomic-swap/rollback model; retire the git+cargo PS1 update path as the canonical Windows update mechanism; confirm Feature 1 (bootstrap install) is already satisfied via docs-only verification; repurpose the existing `test.md` release marker as a real update-verification canary whose content changes per version.
---

# Draft: v0-2-0-install-and-update

## Components (topology ledger)
<!-- Lock the SHAPE before depth. One row per top-level component that can succeed or fail independently. -->
<!-- id | outcome (one line) | status: active|deferred | evidence path -->
- C1 | Feature 1: GitHub-page bootstrap install deploys to both Linux and Windows on first install | active (likely already satisfied - confirm scope) | README.md:1-31, .github/workflows/release.yml, deploy/release/install-wifimic-*.{sh,ps1}
- C2 | Feature 2: Windows self-update via a dedicated `wifimic-client-updater.exe` mirroring the Linux `wifimic_server check-update`/`upgrade` model | active (net-new engineering) | apps/wifimic_server/src/{update_cli.rs,upgrade.rs,upgrade_native.rs}, crates/wifimic_update/src/lib.rs
- C3 | Naming decision: canonical running-client binary name (`wifimic_client.exe` vs the spec's `wifi-client.exe`) | active (owner decision) | apps/wifimic_client/Cargo.toml:2, deploy/windows/update-wifimic-client.ps1:16-21, README.md
- C4 | Disposition of the existing git+cargo-build-based `deploy/windows/update-wifimic-client.ps1` (dev-machine update path) once the compiled updater exists | active (owner decision) | deploy/windows/update-wifimic-client.ps1 (958 lines), docs/release-process.md:27
- C5 | Release pipeline + docs updated to build/package/document the new updater exe | active (follows from C2-C4) | .github/workflows/release.yml, docs/release-process.md, docs/deployment.md, README.md
- C6 | (Informational, out-of-scope by default) `test.md` marker file containing profanity is bundled into every release archive today | deferred unless user opts in | .github/workflows/release.yml:23,45

## Open assumptions (announced defaults)
<!-- Record any default you adopt instead of asking, so the user can veto it at the gate. -->
<!-- assumption | adopted default | rationale | reversible? -->
- Windows updater downloads a pre-built release archive (like Linux) rather than building from source on the target machine | adopted as the plan's baseline design (pending confirmation as part of C4 fork) | mirrors the already-shipped, already-tested Linux `upgrade_native.rs` pattern; avoids requiring git+Rust toolchain on the end-user's Windows machine | reversible (design choice, not yet built)
- Updates stay manual, one-shot commands invoked by the user (no background auto-update) for both platforms | adopted, not asked | matches README's explicit statement that Linux updates are manual-only; nothing in the v0.2.0 note asks for automatic/background updates | reversible
- Test strategy: tests-after per todo (unit + integration), matching the repo's existing convention of a paired implementation+test commit per unit of work (see `apps/wifimic_server/src/upgrade_tests.rs`, `upgrade_test_support.rs`) | adopted, will confirm in brief | strong existing precedent in this exact codebase | n/a (confirmed, not really reversible mid-flight)

## Findings (cited - path:lines)
- README.md:9-31 already documents a working "Install from GitHub" one-liner for both Linux and Windows (`irm ... install-wifimic-windows.ps1 | iex`, `curl ... install-wifimic-linux.sh | bash`), backed by `.github/workflows/release.yml:1-73` which builds and publishes both installers plus checksummed archives on every `vX.Y.Z` tag push. This strongly suggests **Feature 1 of the v0.2.0 note is already shipped** as of the current `main` HEAD.
- README.md:33-41 documents the Linux manual-update commands `wifimic_server check-update` / `wifimic_server upgrade [--tag]`, confirmed implemented in `apps/wifimic_server/src/update_cli.rs`, `upgrade.rs`, `upgrade_native.rs`: downloads the release `.tar.gz` + `.sha256` from GitHub, verifies SHA-256, extracts, atomically swaps the binary, restarts the systemd user unit, with automatic rollback (`upgrade.rs:173-236`, `upgrade_native.rs` full file).
- No equivalent exists on the Windows client today. `apps/wifimic_client/src/{main.rs,control.rs,control_logic.rs}` have zero update/upgrade logic (grep-confirmed; the only "update" hits are unrelated NTP-style clock-offset `CalibrationUpdate` code). There is no `apps/wifimic_client/src/bin/*updater*` file and no `[[bin]]` entry in `apps/wifimic_client/Cargo.toml` beyond the implicit `wifimic_client` binary and the existing `wifimic_control_smoke` dev-tool bin.
- The only existing Windows "update" mechanism is `deploy/windows/update-wifimic-client.ps1` (958 lines) - a PowerShell script that requires `git` + `cargo` on the **target machine**: it does a `git fetch --tags` + `git worktree add --detach` + `cargo build --release -p wifimic_client` from source, then transactionally swaps the Scheduled Task binary with rollback. This is architecturally different from the Linux release-archive-download model and is heavier (dev toolchain required on the end-user's Windows box, `192.168.0.200`).
- `docs/release-process.md:27` currently mandates running this PS1 script as the real-deployment verification step for every version.
- Git history (`.git/logs/refs/heads/main` lines 119-122) shows commit messages "feat(server): add manual check-update/upgrade/status/doctor CLI commands" then "feat(client): add Windows check-update/upgrade via compiled installer" at the current `main` tip - but the **working tree has no trace of the compiled-installer client feature** (confirmed by grep across `apps/wifimic_client/src/**` and by inspecting the abandoned worktree checkout at `%TEMP%\opencode\wifimic-client-update-334b3f5`, which matches the same PS1-only state). Treating on-disk code as ground truth: this feature does not exist yet and is genuinely net-new work, regardless of what the commit message once claimed.
- v0.2.0 note names the two binaries as `wifimic-client-updater.exe` and `wifi-client.exe`. The existing canonical name everywhere in the repo (Cargo package name, README, Scheduled Task identity in `update-wifimic-client.ps1:16-21`, firewall/task docs) is `wifimic_client.exe` / `wifimic-client`. `wifi-client.exe` (missing "mic") does not appear anywhere else in the repo - likely a typo in the note, but a real naming fork since renaming the shipped binary has a wide blast radius (Scheduled Task registration, install/update scripts, README, release archive contents).
- Anomaly (unrelated to this request): `.github/workflows/release.yml:23` and `:45` write a `test.md` file containing the literal string `fuck you!` into both the Windows and Linux release archives on every build; this ships to real installs today. Traced to historical commits `feat(release): bundle test.md marker file in v0.1.5 packages` and `feat(install): deploy bundled test.md marker into install root when present`. Flagged as a discovered defect, not folded into scope unless the user opts in (see C6).

## Decisions (with rationale)
- **D1 (C3 naming): keep `wifimic_client.exe`.** User confirmed the recommended default - `wifi-client.exe` in the note was a typo/shorthand. No binary rename; no touches to Scheduled Task identity, README, or install scripts on this axis.
- **D2 (C1 scope): Feature 1 is treated as already satisfied.** No new install-flow engineering. Plan adds a verification-only todo confirming the two README one-liners still resolve and install correctly, and documents this closure explicitly (so v0.2.0 release notes can cite it).
- **D3 (C4 disposition): the new compiled `wifimic-client-updater.exe` REPLACES `update-wifimic-client.ps1` as the canonical, documented Windows update path.** `docs/release-process.md` step 4 gets rewritten to invoke the new exe instead of the PS1 script. The PS1 script itself is left in the repo (not deleted - out of scope to delete working code) but removed from the mandatory release checklist and README.
- **D4 (C6 test.md marker) - SCOPE EXPANSION per user request:** the existing `test.md` file bundled by `.github/workflows/release.yml` (currently containing "fuck you!" on both Windows and Linux packages) is repurposed as a deliberate **update-verification canary**: its content changes to `suck my dick` for this release-process change, and going forward its content is expected to differ across versions so that a content diff proves a real binary swap occurred (not just "the process runs"). This directly serves `docs/release-process.md`'s existing mandate that a version's effect must be *observably* verified, not just CI-green. New work folded into scope:
  - Update `.github/workflows/release.yml` marker content for both `dist/windows/test.md` and `dist/linux/test.md` from `fuck you!` to `suck my dick`.
  - The new `wifimic-client-updater.exe` reads the installed `test.md` content before and after the swap and reports/logs whether it changed, as an extra automated proof-of-update signal alongside the existing binary-hash/Scheduled-Task health check.
- **D5 (test strategy): tests-after per todo, matching the repo's existing `upgrade_tests.rs` / `upgrade_test_support.rs` pattern** (injectable operations trait + fake ops for deterministic, no-network unit tests), plus the mandatory real two-machine manual verification from `docs/release-process.md` for the actual release.
- **D6 (adopted, not asked - obvious necessity found on second pass): the first-install path must also deploy `wifimic-client-updater.exe`.** Closing Feature 1 as "verify only" (D2) would leave a fresh v0.2.0 install with a running client but NO updater binary present - the update feature would be unusable for anyone who installed after this ships. `deploy/windows/install-wifimic-client.ps1`, `deploy/release/install-wifimic-windows.ps1`, and `.github/workflows/release.yml`'s Windows package step must all be extended to also place `wifimic-client-updater.exe` in `C:\Program Files\wifimic-client\` alongside `wifimic_client.exe`. No alternative is reasonable, so this is folded in as scope, not asked.

## Scope IN
- Verification-only closure of Feature 1 (GitHub-page bootstrap install): confirm both one-liners still work end-to-end; document as done.
- Design + implement `wifimic-client-updater.exe`: a new compiled Rust binary (new `[[bin]]` in `apps/wifimic_client`) that downloads the release archive, verifies SHA-256 (reusing `wifimic_update` crate primitives already used by the server), stops the Scheduled Task, atomically swaps the running `wifimic_client.exe`, restarts the task, verifies health (task state + VB-CABLE endpoint, matching the checks already in `update-wifimic-client.ps1`), with automatic rollback on failure - mirroring `apps/wifimic_server/src/upgrade.rs` + `upgrade_native.rs`.
- Add `check-update` / `upgrade [--tag vX.Y.Z]` CLI surface to the new updater exe, mirroring `wifimic_server`'s `update_cli.rs` wording/exit codes and Traditional-Chinese output strings.
- Repurpose `test.md` as an update-verification canary: change its bundled content (`fuck you!` -> `suck my dick`) in `.github/workflows/release.yml` for both platforms; have the new updater read/compare it before and after swap as an extra proof-of-update signal.
- Update `.github/workflows/release.yml` to build and package the new updater exe into the Windows release archive.
- Update README.md, docs/release-process.md (replace the PS1-based Windows update step with the new exe), docs/deployment.md to document the new Windows update command(s) alongside the existing Linux ones.

## Scope OUT (Must NOT have)
- No automatic/background update checking or scheduled self-update - manual, user-invoked only (matches existing Linux behavior).
- No change to the audio capture/render pipeline, control-plane protocol, or any v0.1.x runtime behavior.
- No rename of the client binary away from `wifimic_client.exe` (D1).
- No deletion of `deploy/windows/update-wifimic-client.ps1` from the repo - it is removed from the mandatory release checklist/docs only, not deleted as a file.
- No new GitHub Pages site (D2) - Feature 1 is verification-only.

## Open questions
2 new forks surfaced on a second, more adversarial pass (see chat - awaiting answers):
6. Admin/privilege model for `wifimic-client-updater.exe`
7. Tray "update available" indicator vs. fully invisible/CLI-only

## Approval gate
status: drafting
next workflow action: on user's explicit okay, rerun `node "<skill-root>/scaffold-plan.mjs" v0-2-0-install-and-update --clear` (without --draft-only) to create `.omo/plans/v0-2-0-install-and-update.md`, run mandatory Metis gap analysis, APPEND todo batches, fill TL;DR last. review_required is false, so after the plan is delivered the user is asked (not decided for them) whether to run the optional dual high-accuracy review before `$start-work`.
<!-- When exploration is exhausted and unknowns are answered, set status: awaiting-approval. -->
<!-- That durable record is the loop guard: on a later turn read it and resume at the gate instead of re-running exploration. -->
