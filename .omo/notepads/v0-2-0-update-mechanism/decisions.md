# Decisions — v0-2-0-update-mechanism

Architectural choices and rationales discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## Todo 2 — updater API surface

- The public error type is `UpdaterError` (`thiserror::Error`) with these stable variants: `InvalidTarget(UpdateError)`, `Download { message: String }`, `InvalidChecksumManifest`, `ChecksumMismatch { expected: String, actual: String }`, `Archive { message: String }`, `MissingExecutable`, `InstallPath { message: String }`, `Backup { message: String }`, `Restore { message: String }`, `Task { operation: &'static str, message: String }`, `Operation { operation: &'static str }`, `Swap { message: String }`, `Endpoint { message: String }`, `HealthCheck { timeout: Duration }`, and `HealthQuery { message: String }`.
- The outcome type is `UpdaterOutcome` with unit variants `NoOp`, `RolledBack`, and `RollbackVerificationFailed`, plus `Installed { tag: String }`. `run_update` has the exact signature `pub fn run_update<O: UpdaterOperations>(operations: &mut O, current_version: &str) -> Result<UpdaterOutcome, UpdaterError>`; rollback outcomes are returned as `Ok(UpdaterOutcome::...)`, while pre-mutation failures remain `Err(UpdaterError)`.
- The exact trait surface is:
  - `fn resolve_latest_tag(&mut self) -> Result<String, UpdaterError>`
  - `fn download_and_verify(&mut self, tag: &str) -> Result<PathBuf, UpdaterError>`
  - `fn backup_current_executable(&mut self, backup_path: &Path) -> Result<(), UpdaterError>`
  - `fn restore_executable(&mut self, backup_path: &Path, install_path: &Path) -> Result<(), UpdaterError>`
  - `fn get_task(&mut self) -> Result<TaskSnapshot, UpdaterError>`
  - `fn disable_task(&mut self) -> Result<(), UpdaterError>`
  - `fn stop_task(&mut self) -> Result<(), UpdaterError>`
  - `fn restore_task(&mut self, snapshot: &TaskSnapshot) -> Result<(), UpdaterError>`
  - `fn enable_task(&mut self) -> Result<(), UpdaterError>`
  - `fn start_task(&mut self) -> Result<(), UpdaterError>`
  - `fn atomic_swap_executable(&mut self, staged: &Path, install_path: &Path) -> Result<(), UpdaterError>`
  - `fn check_render_endpoint_enumerable(&mut self) -> Result<bool, UpdaterError>`
  - `fn wait_for_healthy(&mut self, timeout: Duration) -> Result<bool, UpdaterError>`
- `TaskSnapshot` is public with private `xml: String`, `enabled: bool`, and `running: bool` fields, constructed with `TaskSnapshot::new(xml, enabled, running)` and read through `xml()`, `enabled()`, and `running()`.
- `updater::download_and_verify_release(tag)` is the public helper reserved for Todo 3's native adapter. It downloads `wifimic-windows-x86_64.zip` and its `.sha256` manifest through `wifimic_update::download_release_asset`, verifies SHA-256 with `sha2::Sha256`, rejects absolute/parent-dir/prefix archive paths, extracts with `zip::ZipArchive`, and requires a non-empty `wifimic_client.exe`.
- ZIP extraction stages into a unique directory under `std::env::temp_dir()` named `wifimic_client.stage.<process-id>-<nanosecond-timestamp>`. The orchestration derives the install target as the `wifimic_client.exe` sibling of `std::env::current_exe()` and uses a unique temp backup path named `wifimic_client.backup.<process-id>-<nanosecond-timestamp>`.
- Rollback always attempts, in order, `restore_executable`, `restore_task(pre-update snapshot)`, and `start_task` only when `snapshot.running()` was true. Any rollback seam failure produces `RollbackVerificationFailed`.

## Todo 3 — native updater and UAC resource

- `NativeUpdaterOperations` invokes `schtasks.exe` for the canonical `\wifimic\wifimic-client` task. XML definition capture uses `/Query /TN ... /XML`; lifecycle state uses `/Query /TN ... /FO LIST /V`, reading `Scheduled Task State` and `Status` for enabled/running/ready state. Restore writes the captured XML to a temporary file, recreates the task with `/Create ... /XML ... /F`, then restores enabled state with `/Change`.
- Executable backup, rollback, and installation always target the sibling client at `C:\Program Files\wifimic-client\wifimic_client.exe`; installation and rollback copy to a same-volume sibling temporary path before `std::fs::rename`, so the updater never swaps its own executable.
- `assets/updater-manifest.rc` is compiled only for `wifimic_client_updater` with `embed_resource::compile_for(...).manifest_required()`. The existing tray resource remains optional and applies to the unelevated client binaries; the updater's manifest alone requests `requireAdministrator`.
- UAC cancellation is handled by Windows before `main()` and therefore has no application-level message or file/task mutation. The updater accepts no command-line arguments and waits for Enter after reporting its result.
