# Incident 2026-09-05：Discord 無麥克風聲 — Linux server `start-limit-hit` 永久死亡

- 日期：2026-09-05（中午 12:00 起服務死亡，當晚開 DC 才發現）
- 影響範圍：Linux `192.168.0.210` → Windows `192.168.0.200` UDP 6902 音訊鏈路全斷，Discord `CABLE Output` 無訊號
- 當下止血：`systemctl --user reset-failed wifimic-server && systemctl --user start wifimic-server`（有效，但非根治）
- 基線版本：`v0.2.13`（commit `e26926e`，HEAD == tag，無程式碼髒改）
- 來源：opncd share `yN7EahN3`（Hephaestus session `ses_f8fbb48…`）+ 本機程式碼交叉驗證

## 1. 時間線

1. 中午 Linux 網路短暫中斷（WiFi flap / DHCP renew / PipeWire 重啟其中之一，journal 首錯需再確認）。
2. `wifimic-server.service` 連續啟動失敗 3 次，systemd 進入 `start-limit-hit`。
3. 網路恢復後服務**不再自動重試**，永久死亡。
4. Windows 端正常監聽 UDP 6902、VB-CABLE 兩端點可列舉，但收不到 audio byte。
5. 手動 `reset-failed + start` 後立刻收到 `session_started`，VB-CABLE peak `0.98`（50/50 有訊號）——證明控制平面、路由、PipeWire source 本來都沒壞。

## 2. 根因鏈（三個設計疊加）

### A. `apps/wifimic_server/src/network.rs` 綁死固定 IP（主因）

```rust
pub const LINUX_SERVER_IP = 192.168.0.210; // network.rs:8
UdpSocket::bind(192.168.0.210:6902)        // network.rs:66 bind_at(server_bind_address())
```

- 該位址在網路 flap 瞬間不存在 → `bind()` 回 `EADDRNOTAVAIL`。
- `main.rs:run_server()` 用 `?` 直接把 bind 失敗升級成進程退出，沒有任何重試。
- 安全邊界（`WindowsPeerIp::accepts()` + 防火牆 peer allow/deny）其實與綁定位址無關，綁 `0.0.0.0` 同樣安全。這是「把安全和綁定混在一起」的不必要耦合。

### B. `deploy/systemd/wifimic-server.service` 重啟策略自殺

```ini
Restart=on-failure
RestartSec=5
StartLimitIntervalSec=60
StartLimitBurst=3
```

- 網路斷 15 秒 = 3 次重試用完 = 永久死亡，網路恢復也不會被喚醒。
- 缺 `After=network-online.target` / `Wants=network-online.target`，開機搶在網路前也會走同一條死亡路徑。

### C. 進程內無重試，責任全丟給 systemd

- capture 管線有 `control.rs: CAPTURE_RETRY_INTERVAL=5s` 重試，但**最脆弱的 socket bind 反而零重試**。
- 短暫錯誤（網路抖動）被放大成致命錯誤（進程退出），再被 B 放大成永久錯誤（start-limit）。

## 3. 為何 `reset-failed + start` 有效（假性修復）

| 動作 | 效果 | 沒解決的 |
|---|---|---|
| `reset-failed` | 清掉 StartLimit 計數器 | 下次 flap 照樣累積到 3 次 |
| `start` | 當下網路已恢復，bind 成功 | 沒證明下次 flap 能自癒 |
| peak 50/50 | 證明當下鏈路是通的 | 沒覆蓋「bind 失敗 → 自癒」路徑 |

## 4. 次要風險（本次未爆，但已觀察到）

- Windows 舊版 binary 無 `status`/`doctor` 子命令時，`wifimic_client status` 會被當成 `RunAudio` 啟動常駐 client → 排程原 client + 2 個誤啟 client 同搶 UDP/VB-CABLE。現行 `apps/wifimic_client/src/cli.rs:61` 已回 `Err(Unrecognized)`，但**已部署的舊 binary 仍危險**，且全域無 single-instance mutex。
- Pinned PipeWire source（`capture_types.rs: PINNED_CAPTURE_SOURCE`）同理：PipeWire 未就緒時 `parec` 失敗路徑與 bind 失敗一樣致命，需同樣用「進程內重試」處理（本次 P0 不動，記為 P1）。

## 5. Hotfix 約束（v0.2.13 運行中不得受影響）

- 基線：`HEAD == v0.2.13 == e26926e`，工作區僅 `AGENTS.md` / `CONTEXT.md` / `issue-tracker.md` 三個技能文檔髒改，無程式碼髒改。
- 版本注入：`apps/wifimic_server/build.rs` 經 `WIFIMIC_SERVER_VERSION` / `GITHUB_REF_NAME` 注入，`Cargo.toml` 內 `0.1.0` 僅為佔位——hotfix 不碰 Cargo version，只靠新 tag `v0.2.14` 產生版本。
- 升級路徑：Linux `deploy/linux/update-wifimic-server.sh vX.Y.Z`（原子替換 + 自動回滾）與 Windows `wifimic_client upgrade vX.Y.Z` 均支援顯式版本，hotfix 發布為 pre-release，先用顯式版本在真機驗證，不動 `latest`。
- 在 hotfix 驗證完成前，**不 chạm 兩台運行中機器**：所有變更先在工作區 + `cargo test/clippy` 驗證，通過後才走 `docs/release-process.md` 的推送 → 打 tag → 雙機顯式版本升級 → 端到端人工驗證。

## 6. P0 方案（本次 hotfix 範圍）

1. **Server bind 改 wildcard**（`network.rs`）：`bind()` 改為 `0.0.0.0:6902`，保留 `WindowsPeerIp` 來源過濾與防火牆規則不變；測試同步更新（`network_binds_the_configured_linux_peer_address` 改斷言 wildcard + port）。
2. **Bind 失敗進程內重試**（`main.rs::run_server`）：對 `EADDRNOTAVAIL` / 連動錯誤做有界退避重試（不退出進程），用盡後才回傳錯誤讓 systemd 接手；避免把 5 秒抖動變成進程死亡。
3. **systemd 自癒**（`wifimic-server.service`）：`Restart=always` + `RestartSec=10` + 放寬 `StartLimitIntervalSec=300 / StartLimitBurst=10` + `After=network-online.target …` / `Wants=network-online.target`。
4. **驗證**：`cargo test -p wifimic_server` + `cargo clippy -- -D warnings` 全綠；雙機驗證前不打 tag。

P1（另開 ticket，不在本 hotfix）：Windows single-instance mutex、PipeWire 未就緒進程內重試、`status` 失敗告警 cron、本文件 §9.2 runbook 補 `start-limit-hit` 條目。

## 8. 實施記錄（hotfix/v0.2.14-startlimit-selfheal，基於 v0.2.13）

- 分支：`hotfix/v0.2.14-startlimit-selfheal`（from `v0.2.13 == e26926e`）；運行中兩機未動，僅本機改碼 + `cargo test/clippy`。
- `apps/wifimic_server/src/network.rs`：新增 `wildcard_bind_address()`（`0.0.0.0:6902`），`bind()` 改走 wildcard；保留 `LINUX_SERVER_IP` / `server_bind_address()` 作為環境位址文檔（deployment/firewall 仍引用它）；測試一拆二（環境位址文檔 + wildcard 綁定）。
- `apps/wifimic_server/src/main.rs`：新增 `bind_server_socket()`——僅對 `AddrNotAvailable` / `AddrInUse` 做 30 次 × 2s 進程內重試（≈60s），其他錯誤直接回傳；啟動印一行 `listening on … (environment address …)`（journal 可見，同時讓舊常數保持被使用）。
- `deploy/systemd/wifimic-server.service`：`Restart=always` + `RestartSec=10` + `StartLimitIntervalSec=300 / StartLimitBurst=10` + `After/Wants=network-online.target`。
- 驗證：`cargo test -p wifimic_server` 47 passed（含 2 個新/改名測試）；`cargo clippy -p wifimic_server --all-targets -- -D warnings` 通過；`cargo fmt -p wifimic_server` 已跑。
- 下一步（按 `docs/release-process.md`，待人工確認後執行）：commit → push → tag `v0.2.14`（pre-release）→ 雙機顯式版本升級驗證 → 端到端人工聽音；驗證前不碰 `latest`、不動運行中服務。

## 7. 參考

- `apps/wifimic_server/src/network.rs:5-16,66-75`
- `apps/wifimic_server/src/main.rs:154-229`（`run_server` bind + 主迴圈）
- `apps/wifimic_server/src/control.rs:18-19`（capture 重試已存在，bind 重試缺失的對照）
- `apps/wifimic_server/src/capture_types.rs:6-17`（pinned source）
- `deploy/systemd/wifimic-server.service`（全文）
- `apps/wifimic_client/src/cli.rs:49-62`（未知參數 fail-fast，舊版對照）
- `docs/deployment.md §9.2`、`docs/release-process.md`
