---
slug: v0-2-0-update-mechanism
status: plan-reviewed-approved
intent: clear
review_required: true
plan_path: .omo/plans/v0-2-0-update-mechanism.md
plan_sha256: 9e0eb7fd198a49e938931e1127f8a862bc8af7db75080e6ea3492d9fd0b59782
review_round_id: r4
workspace_root: C:\Users\Daniel\Documents\opencode\wifimic
pending-action: hand off the approved plan to a separate execution session only when the user explicitly starts work
convergence:
  max_rounds: 8
  max_rounds_override: explicit_user_request ("完成計劃後持續進行高精度雙審，最高限制八輪")
  round_count: 4
  ledger_freeze_after_round: 1
  accepted_blockers:
    - "R1-B1/B2/B3 (independent): embed_resource::compile_for signature, executable backup/restore + TaskSnapshot rollback seam, and run_update's current_version source. All FIXED in round-1 edit, CONFIRMED fixed by independent's round-2 APPROVE."
    - "R1-B4 (momus): round-1 INCONCLUSIVE was a tooling limitation, not a content finding; resolved procedurally in round 2 by allowing direct-Read identity binding."
    - "R2-B1 (momus, explicit_requirement_or_accepted_decision): todo 5's repo-wide grep for residual PS1 references would always self-match this very plan/draft (which legitimately describe the deletion in prose), making the acceptance criterion unexecutable as written. FIXED: all 3 grep invocations in todo 5 (the 'What to do' final check, Acceptance criteria, and QA failure line) now add --exclude-dir=.omo --exclude-dir=.git --exclude-dir=target."
    - "R2-B2 (momus, explicit_requirement_or_accepted_decision): F4's 'byte-identical to pre-plan state' had no baseline commit reference, making the check unexecutable. FIXED: a preflight records `git rev-parse HEAD` to .omo/evidence/plan-baseline-commit-v0-2-0-update-mechanism.txt; F4 runs `git diff --exit-code <baseline-sha> HEAD -- deploy/release/install-wifimic-windows.ps1 deploy/release/install-wifimic-linux.sh` against that recorded baseline."
    - "R3-B1 (momus, explicit_requirement_or_accepted_decision): the baseline was required before mutations but was embedded in todo 1 while todos 1 and 2 were parallel Wave-1 starts, so todo 2 could mutate first. FIXED: moved capture to an explicit mandatory preflight before either Wave-1 todo may be dispatched; todo 1 now only confirms the preflight artifact exists and must not overwrite it."
  non_blocking_notes:
    - "independent (r1): zip crate Cargo.lock update is implicit in normal cargo build workflow, not a separate action item."
    - "independent (r2): resource compilation correctly preserves tray-icon.rc's .manifest_optional() behavior, avoiding elevation contamination - confirmed working as intended."
    - "momus + independent (r4): F4 still says the baseline was 'recorded by todo 1', though the mandatory dispatcher preflight now writes it. This is a nonblocking stale attribution: F4 reads the correct required artifact and its executable diff command is unchanged. The plan is approved with this note; no plan edit is made because approval-with-notes counts as approval and an edit would invalidate both review receipts."
  round_history:
    r1: {momus: INCONCLUSIVE (tooling), independent: CHANGES_REQUESTED (3 blockers, all fixed)}
    r2: {momus: CHANGES_REQUESTED (2 blockers, both fixed), independent: APPROVE (must re-run r3 since plan changed after r2's approval, per "any plan change invalidates both lanes")}
    r3: {momus: CHANGES_REQUESTED (1 blocker, fixed), independent: APPROVE (must re-run r4 since plan changed after r3's approval, per "any plan change invalidates both lanes")}
    r4: {momus: APPROVE_WITH_NOTE, independent: APPROVE}
review:
  momus:
    status: approved-with-note
    workspace_root: C:\Users\Daniel\Documents\opencode\wifimic
    runtime_home: null
    target: .omo/plans/v0-2-0-update-mechanism.md
    round_id: r4
    plan_sha256: 9e0eb7fd198a49e938931e1127f8a862bc8af7db75080e6ea3492d9fd0b59782
    launch_id: r4-momus-l1
    session: ses_fc2d9a77bffexsOrvdh6kVtgQ5
    result: APPROVE_WITH_NOTE (the preflight fixes R3-B1; stale F4 attribution is nonblocking)
  independent:
    status: approved
    workspace_root: C:\Users\Daniel\Documents\opencode\wifimic
    runtime_home: null
    target: .omo/plans/v0-2-0-update-mechanism.md
    round_id: r4
    plan_sha256: 9e0eb7fd198a49e938931e1127f8a862bc8af7db75080e6ea3492d9fd0b59782
    launch_id: r4-independent-l1
    session: ses_fc2d9a778ffe0MSebthMldp2kG
    result: APPROVE (the preflight fixes R3-B1; no eligible blockers)
approach: 直接依 GitHub issue #1（父史詩）與其 5 個子任務 #2-#6 的既有規格（已透過先前的 /grill-with-docs 訪談與已核准的 ADR 0001 定案，無殘留待決分岉）合成一份 ulw-plan 格式的 decision-complete 計畫；不重問已被 issue 回答過的問題，只把 issue 內容轉譯成含完整 References／Acceptance／QA／依賴矩陣的 Todos，並在核准後執行高精度雙審（momus + oracle）直到通過或達 5 輪上限。
---

# Draft: v0-2-0-update-mechanism

## Components (topology ledger)
<!-- Lock the SHAPE before depth. One row per top-level component that can succeed or fail independently. -->
<!-- id | outcome (one line) | status: active|deferred | evidence path -->
- C1 | Issue #2 — Linux `wifimic_server check-update` 更名為 `update`（行為不變，`check-update` 變成未辨識指令） | active | apps/wifimic_server/src/cli.rs:14,54,157; apps/wifimic_server/src/update_cli.rs; README.md
- C2 | Issue #3 — 新增 `wifimic_client` 底下、以明確 Cargo `[[bin]]` target 宣告的 `wifimic_client_updater` bin 骨架 + `UpdaterOperations` trait + `FakeUpdaterOperations`，核心更新流程以假物件單元測試涵蓋 | active | apps/wifimic_client/Cargo.toml; apps/wifimic_client/src/bin/wifimic_control_smoke.rs（自動探索前例，但 issue #1 明訂仍需 `[[bin]]`）; apps/wifimic_server/src/upgrade.rs:85-169（UpgradeOperations 前例）; apps/wifimic_server/src/upgrade_test_support.rs（FakeUpgradeOperations 前例）
- C3 | Issue #4 — `NativeUpdaterOperations` 真實實作（schtasks.exe、原子替換、`enumerate_render_endpoints()`、`wifimic_update` crate 下載驗證）+ `main()` 串接內嵌 UAC manifest 與 console 狀態輸出 | active (blocked by C2) | apps/wifimic_client/build.rs:1-4（embed-resource 既有前例，`.manifest_optional()`); apps/wifimic_client/src/render_windows_endpoints.rs:71-104; crates/wifimic_update/src/lib.rs:126-186; apps/wifimic_server/src/upgrade_native.rs
- C4 | Issue #5 — `install-wifimic-client.ps1` 安裝時一併複製 `wifimic_client_updater.exe`；`.github/workflows/release.yml` 建置並打包進 zip | active (blocked by C3) | deploy/windows/install-wifimic-client.ps1:501-576（既有 marker-file CopyFile 前例可直接沿用同一模式）; .github/workflows/release.yml:12-31
- C5 | Issue #6 — 刪除 `deploy/windows/update-wifimic-client.ps1`；改寫 `docs/release-process.md` §4 與 `docs/deployment.md` §7（含 §1 對照表與 §12 參考檔案兩處殘留引用） | active (blocked by C4) | docs/release-process.md:26-27,66; docs/deployment.md:32,464-548,728
- C6 | (資訊性 / 不在範圍) `.github/workflows/release.yml` 目前在兩平台封裝的 `test.md` 內容為 `fuck you!` | deferred — 不在 issue #1 範圍內，不主動處理 | .github/workflows/release.yml:23,45

## Open assumptions (announced defaults)
<!-- Record any default you adopt instead of asking, so the user can veto it at the gate. -->
<!-- assumption | adopted default | rationale | reversible? -->
- 完成每個子任務（issue #2-#6）對應的 todo 後，於該 todo 的 commit 訊息中一併 `gh issue close <n> --comment "..."` 關閉對應 GitHub issue | adopted，未詢問 | 符合 `docs/agents/issue-tracker.md` 既有慣例（Resolve: comment 後 close）；issue 本身即是本計畫的規格來源 | reversible（純追蹤動作，不影響程式碼）
- 測試策略：tests-after，逐 todo 隨實作補齊單元測試，比照 repo 既有 `upgrade_tests.rs` / `upgrade_test_support.rs` 的 `FailurePoint` 假物件故障注入模式 | adopted | issue #3/#4 的 Testing Decisions 段落已明確指定此模式為前例 | n/a（已由 issue 決定）
- 不處理 C6（`test.md` 內容為不雅字串）：維持現狀，不在本計畫範圍內修正 | adopted | issue #1 本文與其 5 個子任務皆未提及 `test.md`；先前 `.omo/drafts/v0-2-0-install-and-update.md` 草稿曾把它納入範圍（D4），但那是舊、未核准、與目前 issue #1 衝突的草稿，issue #1 才是目前的權威規格來源 | reversible（若使用者仍想處理，可另開新請求）

## Findings (cited - path:lines)
- `apps/wifimic_server/src/cli.rs:1-113` 目前的 `Command::CheckUpdate`／字串比對 `"check-update"` 尚未更名；`update_cli.rs:1-92`（`TagDiscovery`、`CheckUpdateOutcome`、`run_check_update`、`render_check_update`、`check_update_exit_code`）的邏輯本身完全不需改動，只需要 cli.rs 的列舉/字串/測試改名（confirms issue #2 acceptance criteria 是純改名，非邏輯變更）。
- `apps/wifimic_server/src/upgrade.rs:84-112`（`UpgradeOperations` trait 的 9 個方法）與 `upgrade_test_support.rs:1-128`（`FakeUpgradeOperations` + `FailurePoint` 列舉：Stop/Swap/Restart/Health）是 issue #3 明確指名要比照的既有、已測試模式；`wifimic_client_updater` 的 `UpdaterOperations` trait 需要對應但不同的方法集合（resolve latest tag、download+verify、get/disable/stop/restore/enable/start 排程工作、atomic swap、檢查 render endpoint、等待健康），比 `UpgradeOperations` 多了排程工作生命週期與端點檢查兩類操作。
- `apps/wifimic_client/Cargo.toml:1-27` 已有 `[build-dependencies] embed-resource = "3.0"` 與 `apps/wifimic_client/build.rs:1-4` 對 `assets/tray-icon.rc` 呼叫 `.manifest_optional()`；`wifimic_client_updater` 需要一份自己的 `.rc`/manifest（`requireAdministrator`），可沿用同一 `embed_resource::compile` 機制，只是換一個獨立的 rc 檔與 exe 目標，不需要新增 build-dependency。
- `apps/wifimic_client/src/bin/wifimic_control_smoke.rs` 已存在，證實 `apps/wifimic_client` 允許透過 `src/bin/*.rs` 自動探索新增二進位檔（Cargo.toml 未見任何 `[[bin]]` 區塊，皆為自動發現），`wifimic_client_updater` 可直接以 `apps/wifimic_client/src/bin/wifimic_client_updater.rs`（或子模組）方式新增，不需修改 Cargo.toml 的 package 區段。
- Metis 缺口分析確認：上述自動探索前例在技術上成立，但 issue #1 的 Implementation Decisions 明訂「New Cargo `[[bin]]` target」，故計畫必須在 `apps/wifimic_client/Cargo.toml` 加入顯式 `[[bin]]` target，不可以只依賴自動探索。
- `apps/wifimic_client/src/render_windows_endpoints.rs:71-104` 的 `enumerate_render_endpoints()` 已存在且公開（`pub fn`），可直接被 `NativeUpdaterOperations` 呼叫做端點健康檢查，不需重新實作。
- `crates/wifimic_update/src/lib.rs:121-186` 的 `discover_latest_tag()`／`download_release_asset(tag, asset)` 已是平台無關的共用 crate，`wifimic_client_updater` 必須把 `wifimic_update` 加入 `apps/wifimic_client/Cargo.toml` 依賴，並直接使用它下載 release asset；但 `download_release_asset()` **只會取回 bytes**，不會自行下載 checksum manifest、驗證 SHA-256、解壓 ZIP、阻擋 ZIP path traversal，這些仍必須由 Windows native updater 以對等於 `apps/wifimic_server/src/upgrade_native.rs` 的流程明確實作與測試。
- `deploy/windows/install-wifimic-client.ps1:571-576` 已有「從 `$source`（client 執行檔）同目錄複製 marker 檔（`test.md`）到安裝目錄」的既有結構可作為前例，但它是**可選**複製且 rollback 只追蹤 `MarkerChanged`；issue #5 的 updater 是必要安裝成品，不能直接複製該段。計畫必須新增 required sibling-file 驗證、updater 的 prior-file capture、changed-state、restore/remove、rollback verification，並以 TestMode/DryRun 假操作驗證，避免缺檔被靜默略過或安裝失敗留下孤兒 updater。
- `deploy/release/install-wifimic-windows.ps1:11-48` 只是解壓縮 release zip 後呼叫 `install-wifimic-client.ps1 -ClientExecutable $client`；只要 `wifimic_client_updater.exe` 與 `wifimic_client.exe` 一起被打包進同一個 zip（issue #5 對 `.github/workflows/release.yml` 的要求）並被 `install-wifimic-client.ps1` 從同目錄複製，這支 bootstrap 腳本本身**不需要任何修改**（新發現，非 issue 明文要求，但屬於「發現且應折疊進計畫」的完整性檢查項）。
- `.github/workflows/release.yml:12-31`（`package-windows` job）目前只複製 `wifimic_client.exe` + `install-wifimic-client.ps1` + 寫入 `test.md`；issue #5 要求新增一行 `cargo build` 目標與一行 `Copy-Item` 把 `wifimic_client_updater.exe` 納入同一個 zip。
- `docs/release-process.md:26-27`（§4 Windows 強制部署步驟，呼叫 `update-wifimic-client.ps1 -Tag vX.Y.Z -AcceptHostMutation`）與 `docs/deployment.md:464-548`（§7 全節，含 §7.1-§7.6 的 PS1 型流程）、`docs/deployment.md:32`（§1 對照表的「Windows 更新腳本」列）、`docs/deployment.md:728`（§12 參考檔案列表）— 這 4 處都引用即將刪除的 `deploy/windows/update-wifimic-client.ps1`，issue #6 本文只明文列出 release-process.md §4 與 deployment.md §7，deployment.md 的 §1 對照表與 §12 參考檔案列表是額外發現的殘留引用，會一併折疊進同一個 todo（否則 `grep -r update-wifimic-client.ps1 docs/` 驗證會失敗）。
- ADR `docs/adr/0001-windows-update-moves-from-source-build-to-self-updater-binary.md` 與 `CONTEXT.md` 已經記錄這些決策為 accepted／已定案文件，本計畫不需要再產生或修改這兩份文件，只需要讓程式碼與既有腳本追上文件已經寫定的決策。
- 舊草稿 `.omo/drafts/v0-2-0-install-and-update.md`（status: drafting，從未核准）與目前 issue #1 存在真實衝突：舊草稿的 D3 決定「保留 PS1 腳本、只從強制清單移除」，但 issue #1／ADR／issue #6 明確決定「整支刪除」；舊草稿的 D4 把 `test.md` 內容置換納入範圍，issue #1 完全未提及。本計畫以 issue #1 為準，視舊草稿為已被取代的探索紀錄，不沿用其衝突決策。

## Decisions (with rationale)
- **D1（採用 issue #1 的既有規格作為唯一權威來源）**：不重新訪談任何在 issue #1／#2-#6／ADR 0001／CONTEXT.md 中已經回答的問題。理由：這些文件本身就是先前 `/grill-with-docs` 高精度訪談的產出並已核准（ADR status: accepted），重問等於讓使用者回答自己已經回答過的問題。
- **D2（子任務 1:1 對應計畫 todo，Implementation+Test 合一）**：issue #2 → todo 1；issue #3 → todo 2；issue #4 → todo 3（依賴 todo 2）；issue #5 → todo 4（依賴 todo 3）；issue #6 → todo 5（依賴 todo 4）。理由：5 個子 issue 本身已經是決策完整、依賴關係明確（#3→#4→#5→#6，#2 獨立）的最小可獨立驗收單元，強行再切分只會打散原本審核過的驗收邊界。
- **D3（額外折疊：deployment.md §1 對照表列 + §12 參考檔案列 + install-wifimic-windows.ps1 完整性確認）**：折疊進 todo 5（issue #6）與 todo 4（issue #5）的驗收範圍，理由見上方 Findings 對應段落——這些是探索發現的殘留引用/完整性缺口，不是新功能。
- **D4（C6 `test.md` 不雅字串）保持不在範圍內**：理由見 Open assumptions；舊草稿的範圍擴張（D4）不隨本計畫沿用。
- **D5（測試/QA 策略）**：tests-after，`cargo test` 為主要驗證手段；`wifimic_client_updater` 的 Native 實作（schtasks.exe、真實檔案系統）比照 `upgrade_native.rs`/`install-wifimic-client.ps1` 既有慣例，不寫單元測試，改為 issue #4 acceptance criteria 明訂的「真實機器手動雙擊驗證」，對應本計畫 Final verification wave 的 F3（real manual QA），並比照 `docs/release-process.md` 的強制真實兩機部署驗證流程。
- **D6（Metis 已接受且可逆的補強）**：todo 2/3 顯式納入 `wifimic_update` Cargo dependency、SHA-256 manifest 驗證、受限 ZIP 解壓（拒絕 traversal、缺少/空的 `wifimic_client.exe`）與 updater-target-specific `embed-resource::compile_for(...).manifest_required()`；理由：這些不是新產品能力，而是 issue #1 已要求的「下載、驗證、解壓」及 requireAdministrator artifact 的必要可驗收細節。
- **D7（Metis 已接受的初次 bootstrap 預設）**：既有 v0.1.x 安裝不會已有 `wifimic_client_updater.exe`，故 v0.2.0 第一次 Windows 更新的文件流程預設為「重新執行已驗證的 release installer 一次，安裝 updater；其後才使用 updater」。此行為必須在 todo 4/5 的 acceptance 與文件中明載；不修改 bootstrap wrapper，前提是 install script 把 sibling updater 視為必需檔並可交易性 rollback。

## Scope IN
- 依 issue #2：`apps/wifimic_server/src/cli.rs` 的 `Command::CheckUpdate`/`"check-update"` 更名為 `Command::Update`/`"update"`，`CliParseError::Unrecognized` 覆蓋舊名；`cli.rs` 既有 table-driven 測試新增/更新對應案例；README.md 等文件字樣同步更新。
- 依 issue #3：在 `apps/wifimic_client/Cargo.toml` 新增明確 `[[bin]]` target 與 `wifimic_update` dependency，新增 `apps/wifimic_client/src/bin/wifimic_client_updater.rs`（或等效模組結構）與支援模組，定義 `UpdaterOperations` trait（resolve latest tag、download+verify、get/disable/stop/restore/enable/start 排程工作、atomic swap 執行檔、檢查 render endpoint、等待健康）+ `FakeUpdaterOperations`（`FailurePoint` 故障注入）+ 核心更新流程函式，以 `cargo test` 完整涵蓋已是最新版本（零 Scheduled Task/檔案 side effect）／乾淨成功更新／每個 FailurePoint 的精確 operation sequence、rollback 與 rollback-verification failure 的可辨識結果；測試不得碰觸真實 Scheduled Task 或檔案系統，尚不接真實 Windows API。
- 依 issue #4：`NativeUpdaterOperations` 實作 `UpdaterOperations` 的所有方法（`schtasks.exe`、`wifimic_update::discover_latest_tag`/`download_release_asset`、checksum manifest SHA-256 驗證、拒絕 ZIP traversal 的 ZIP 解壓、既有 `enumerate_render_endpoints()`）；`main()` 為 updater target 專屬地內嵌 requireAdministrator UAC manifest（以 `embed-resource::compile_for(...).manifest_required()`，不可污染既有 client/smoke binary）、console 狀態文字輸出（檢查中/已是最新/更新中/成功/失敗，結束前等待按鍵）；不支援任何命令列參數／`--tag`。
- 依 issue #5：`install-wifimic-client.ps1` 以既有 marker-file 結構為前例、但把 `wifimic_client_updater.exe` 視為必要 sibling，複製到 `C:\Program Files\wifimic-client\` 並在 TestMode/DryRun/Fake operations、prior capture、rollback remove/restore/verification 中完整追蹤；`.github/workflows/release.yml` 的 `package-windows` job 新增建置與打包 `wifimic_client_updater.exe` 進 `wifimic-windows-x86_64.zip`。
- 依 issue #6：刪除 `deploy/windows/update-wifimic-client.ps1`；改寫 `docs/release-process.md` §4（Windows 部署驗證步驟改呼叫新 exe，並註明只能驗證「latest」而非精確 tag）；改寫 `docs/deployment.md` §7（全節，含 §7.1-§7.6 對應新 exe 的先決條件/執行/驗證），並同步修正 §1 對照表列與 §12 參考檔案列表中對舊 PS1 腳本的殘留引用；repo 內（含 docs、CI 設定）搜尋不到任何殘留的 `update-wifimic-client.ps1` 引用。
- 每個 todo 完成後於 commit 訊息/QA 證據中記錄對應 GitHub issue 編號，並在該 todo 內建議以 `gh issue close <n> --comment "..."` 關閉對應 issue（issue #1 本身待 5 個子 issue 全部關閉、且 Final verification wave 全數 APPROVE 後才關閉）。

## Scope OUT (Must NOT have)
- 不新增背景/自動更新排程（兩端都維持人工手動觸發，符合 `CONTEXT.md`「Manual Update」定義）。
- 不修改 `wifimic_server upgrade [--tag]` 與 `doctor` 子指令的行為或底層模組。
- 不為 `wifimic_client_updater.exe` 新增任何命令列參數／`--tag` 支援（ADR 明確接受的 v0.2.0 範圍縮減）。
- 不新增 Windows「doctor」風格診斷指令。
- 不保留 `check-update` 舊名的相容別名。
- 不刪除 `deploy/windows/update-wifimic-client.ps1` 以外的任何現有腳本/程式碼。
- 不處理 `.github/workflows/release.yml` 中 `test.md` 標記檔內容為不雅字串的既有問題（C6，見 Open assumptions）。
- 不新增或修改初次安裝流程本身（`README.md` 一鍵指令、`install-wifimic-windows.ps1`、`install-wifimic-linux.sh`）除了 install-wifimic-client.ps1 內新增一個檔案複製動作之外。
- 不沿用舊草稿 `.omo/drafts/v0-2-0-install-and-update.md` 的任何與 issue #1 衝突的決策（保留 PS1、test.md 內容置換）。

## Open questions
唯一阻擋分岉（Metis 找到的 Windows 平台不可實作矛盾）：issue #1 同時要求 updater exe 內嵌 `requireAdministrator` manifest，及「使用者拒絕 UAC 後 updater 自己印出清楚訊息」。前者使 Windows loader 在程式進入 `main()` 前就停止執行，故單一 exe 不可能在拒絕後自行輸出訊息。需要使用者選擇：
1. **（建議，最符合 issue #1 的單一 updater-exe 架構）接受 Windows 原生的 UAC 取消提示為清楚結果**：維持一個 `wifimic_client_updater.exe`、內嵌 `requireAdministrator`；拒絕時程式不會執行/不修改任何狀態，計畫與文件明確說明由 Windows UAC 對話框提供取消回饋。
2. **新增 unelevated launcher + elevated worker**：launcher 可以在 ShellExecuteEx 取消時印字，但這會從「一個 updater exe」變成兩個使用者可見/部署的程式，超出 issue #1 已定案架構。

其餘 Metis 發現皆已以 D6/D7 的可逆預設折疊，無需再問：既有安裝首次改版時重跑 release installer 一次；不新增 Authenticode/SmartScreen/compliance 支援（未指定、無新付費服務）；Windows x86_64、單一互動式操作者、GitHub 公開 release HTTPS、無 updater self-update 皆維持 issue #1 現有範圍。

## Approval gate
status: awaiting-approval
next workflow action: 使用者針對唯一 UAC 分岉選擇後（回覆選項 1 即同時代表依既有 issue #1 核准），APPEND 5 個 todo + Final verification wave（F1-F4）到已建立的 `.omo/plans/v0-2-0-update-mechanism.md`，最後填 TL;DR。review_required 為 true（使用者已明確要求「高精度雙審直到通過」），故計畫寫完後立即執行 momus + oracle 雙審，依 5 輪上限的收斂規則修正直到通過或達上限後停下詢問使用者。
<!-- When exploration is exhausted and unknowns are answered, set status: awaiting-approval. -->
<!-- That durable record is the loop guard: on a later turn read it and resume at the gate instead of re-running exploration. -->
