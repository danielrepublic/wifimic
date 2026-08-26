# wifimic 版本發布流程（強制）

本文件定義「一個版本何時算完成」。**程式碼變更完成、測試通過、甚至 CI 建置成功，都不構成版本完成的證明。** 唯有在真實的兩機環境（Linux `192.168.0.210` / Windows `192.168.0.200`）上實際部署並人工驗證過，該版本才算完成。

> 本規則自 v0.1.8 起生效，適用於本專案的所有後續版本。

---

## 1. 強制步驟

每次完成一個新版本時，必須依序完成以下步驟，缺一不可：

1. **推送程式碼**：`git push`，確保 `origin/main` 與本地一致。
2. **建立並推送版本標籤**：
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
   這會觸發 `.github/workflows/release.yml`，自動建置 Windows／Linux 兩份成品並發布 GitHub Release。
3. **確認 Release workflow 成功**：
   ```bash
   gh run list --workflow=release.yml --limit 1
   gh release view vX.Y.Z
   ```
   確認 6 個成品皆已上傳：`install-wifimic-windows.ps1`、`wifimic-windows-x86_64.zip`（含 `.sha256`）、`install-wifimic-linux.sh`、`wifimic-linux-x86_64.tar.gz`（含 `.sha256`）。
4. **實際部署（不得省略，即使本版只改了一端）**：
   - **Windows 端**（真實用戶端機器 `192.168.0.200`）：在系統管理員互動式工作階段執行 `wifimic_client.exe upgrade --tag vX.Y.Z`（尚未安裝過則使用 README 的一鍵安裝指令；其底層會呼叫 `wifimic_client_installer.exe install`），並依 `docs/deployment.md` 第 7.6 節驗證。
   - **Linux 端**（真實伺服器 `192.168.0.210`）：執行 `WIFIMIC_CONTROL_SMOKE_HELPER=<path> deploy/linux/update-wifimic-server.sh vX.Y.Z`，並依 `docs/deployment.md` 第 6.5 節驗證。
5. **端到端人工驗證**（`docs/deployment.md` 第 8.3 節）：
   - Linux 端 `journalctl --user -u wifimic-server -f` 觀察 `parec` 讀取與 UDP 發送、控制平面 Start/Heartbeat Ack。
   - Windows 端登入觸發排程工作或手動 `schtasks /Run /TN '\wifimic\wifimic-client'`。
   - 確認 VB-CABLE Output 端有聲音輸出，且本版變更的可觀察效果（例如音量、延遲）確實存在，而非僅「程式能跑」。
6. 只有 1～5 全部通過，才可以將該版本標記為「完成」並回報使用者。若任一步驟失敗，先排查並修正，不得跳過或以「CI 綠燈」代替真實部署驗證。

---

## 2. 快速檢查清單

- [ ] 程式碼已推送至 `origin/main`
- [ ] 標籤 `vX.Y.Z` 已推送
- [ ] `gh release view vX.Y.Z` 顯示 6 個成品皆已上傳，且與本地建置的 SHA256 一致
- [ ] Windows 端（192.168.0.200）已實際更新／安裝並通過健康檢查
- [ ] Linux 端（192.168.0.210）已實際更新並通過健康檢查
- [ ] 端到端人工音訊驗證通過，且本版變更的效果可實際觀察到

---

## 3. 理由

CI 建置成功只證明程式碼可在 GitHub Actions 的乾淨環境中編譯，不代表：

- 真實 Wi-Fi 網路下的封包遺失/延遲表現正常
- 真實音訊裝置（PipeWire 捕獲源、VB-CABLE 端點）仍可列舉
- 真實防火牆規則、排程工作、systemd 使用者單元仍正確生效
- 本版變更（例如音量調整、延遲修正）在人耳/實際訊號上確實有效

v0.1.0–v0.1.7 皆已手動驗證過完整部署流程；本文件將其列為每版必須的強制步驟，避免未來版本略過真實部署測試就宣稱「完成」。

---

## 4. 參考檔案

- `docs/deployment.md` — 完整安裝／更新／驗證步驟（本文件第 4、5 步引用其章節）
- `docs/deployment-linux.md` — Linux 伺服器詳細部署指南
- `.github/workflows/release.yml` — 標籤觸發的自動建置與發布流程
- `apps/wifimic_client/src/bin/wifimic_client_installer.rs` — Windows 用戶端安裝器命令列入口
- `deploy/linux/update-wifimic-server.sh` — Linux 伺服器更新腳本
