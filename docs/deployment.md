# wifimic 兩機端到端部署指南

本文件整合 Linux 伺服器部署、Windows 用戶端安裝、Linux 伺服器更新、Windows 用戶端更新四大流程，針對固定對等端 `192.168.0.210`（Linux）與 `192.168.0.200`（Windows）、UDP 6902、固定 VB-CABLE Input 端點、固定 PipeWire 捕獲源 `alsa_input.pci-0000_00_1b.0.analog-stereo` 的實際成品。

> **重要**：Linux 端採用 **使用者層級 systemd 單元** 搭配 `loginctl enable-linger daniel`，使 PipeWire 維持在使用者工作階段中；Windows 端採用 **互動式登入觸發的排程工作**，以 `InteractiveToken` 權限直接啟動 `wifimic_client.exe`。兩端皆無系統層級服務、無背景自動更新、無密碼持久化。

---

## 1. 關鍵數值對照表

| 項目 | 數值 | 來源 |
|------|------|------|
| Linux 伺服器 IP | `192.168.0.210` | 部署環境固定、`apps/wifimic_server/src/network.rs:16` |
| Windows 對等端 IP | `192.168.0.200` | `apps/wifimic_server/src/network.rs:16`、`deploy/linux/wifimic-server-firewall.sh:4` |
| UDP 埠號 | `6902` | `apps/wifimic_server/src/network.rs:5`、`deploy/linux/wifimic-server-firewall.sh:5` |
| 固定 PipeWire 捕獲源 | `alsa_input.pci-0000_00_1b.0.analog-stereo` | `apps/wifimic_server/src/capture_types.rs:6` |
| `parec` 參數 | `--raw --format=s16le --rate=48000 --channels=1 --latency-msec=5 --process-time-msec=5 --device=alsa_input.pci-0000_00_1b.0.analog-stereo` | `apps/wifimic_server/src/capture_types.rs:9-17` |
| Windows 安裝根目錄 | `C:\Program Files\wifimic-client` | `deploy/windows/install-wifimic-client.ps1:18` |
| Windows 執行檔名稱 | `wifimic_client.exe` | `deploy/windows/install-wifimic-client.ps1:19` |
| Windows 排程工作資料夾 | `\wifimic\` | `deploy/windows/install-wifimic-client.ps1:20` |
| Windows 排程工作名稱 | `wifimic-client` | `deploy/windows/install-wifimic-client.ps1:21` |
| Windows 排程工作完整路徑 | `\wifimic\wifimic-client` | `deploy/windows/install-wifimic-client.ps1:22` |
| Windows 防火牆顯示名稱 | `wifimic-client` | `deploy/windows/install-wifimic-client.ps1:23` |
| Windows 防火牆遠端位址 | `192.168.0.210/32` | `deploy/windows/install-wifimic-client.ps1:24` |
| Windows 防火牆通訊埠 | `6902` (UDP) | `deploy/windows/install-wifimic-client.ps1:25` |
| Windows 固定渲染端點 | `CABLE Input (VB-Audio Virtual Cable)` | `deploy/windows/install-wifimic-client.ps1:26` |
| Linux 服務名稱 | `wifimic-server` | `deploy/systemd/wifimic-server.service` |
| Linux 二進位檔路徑 | `~/.local/bin/wifimic_server` | systemd 單元 `ExecStart` |
| Linux systemd 使用者單元路徑 | `~/.config/systemd/user/wifimic-server.service` | `deploy/linux/update-wifimic-server.sh:7` |
| Linux 更新腳本 | `deploy/linux/update-wifimic-server.sh` | — |
| Windows 安裝腳本 | `deploy/windows/install-wifimic-client.ps1` | — |
| Windows 更新腳本 | `deploy/windows/update-wifimic-client.ps1` | — |

---

## 2. 先決條件

### 2.1 Linux 伺服器端 (`arch-daniel` / `192.168.0.210`)

- Arch Linux（或衍生發行版）已安裝 `base-devel`、`rustup`、`pipewire`、`wireplumber`、`ufw` 或 `nftables`/`iptables` 其中一套防火牆後端
- 使用者 `daniel` 具備 `sudo` 權限
- 區域網路固定 IP：`192.168.0.210`
- UDP 6902 僅允許 `192.168.0.200` 存取
- 已安裝 VB-Audio Virtual Cable（Windows 端），且 `CABLE Input (VB-Audio Virtual Cable)` 端點可被 PnP AudioEndpoint 列舉

### 2.2 Windows 用戶端端 (`192.168.0.200`)

- Windows 10/11，PowerShell 5.1
- 已安裝 VB-Audio Virtual Cable，`CABLE Input (VB-Audio Virtual Cable)` 端點存在
- 具備系統管理員權限的互動式工作階段（原生安裝/更新需要）
- Git for Windows（提供 `git.exe`、`bash.exe`）
- Rust 工具鏈（`cargo.exe` 可用於建置）

### 2.3 共同先決條件

- 私有 Git 倉庫可存取（`git@github.com:<your-account>/wifimic.git`）
- 兩機時鐘同步（NTP），避免協定時間戳偏差
- 無其他防火牆服務同時啟用（Linux 端參見第 6.2 節）

---

## 3. 取得私有倉庫原始碼

### 3.1 Linux 伺服器端

```bash
# 於任意目錄（建議 ~/src）克隆私有倉庫
git clone git@github.com:<your-account>/wifimic.git
cd wifimic
```

> 若已有本地副本，改用 `git fetch origin && git checkout main && git pull` 更新至最新提交。

### 3.2 Windows 用戶端端

```powershell
# 於任意目錄（建議 C:\src）克隆私有倉庫
git clone git@github.com:<your-account>/wifimic.git
cd wifimic
```

> 若已有本地副本，改用 `git fetch origin && git checkout main && git pull` 更新至最新提交。

---

## 4. Linux 伺服器部署（首次）

完整細節請參照 `docs/deployment-linux.md`；此處僅列出有序步驟與關鍵驗證指令。

### 4.1 本地 Rust 建置與安裝

```bash
# 確認工具鏈版本（workspace 指定 1.97.1）
rustup show

# 建置發布版本
cargo build --release --bin wifimic_server

# 安裝至使用者本地 bin 目錄（systemd 單元參照此路徑）
mkdir -p ~/.local/bin
cp target/release/wifimic_server ~/.local/bin/
```

驗證二進位檔：

```bash
file ~/.local/bin/wifimic_server
# 輸出應包含 ELF 64-bit LSB executable, x86-64
```

### 4.2 固定 PipeWire 捕獲源驗證

伺服器**僅**接受 `alsa_input.pci-0000_00_1b.0.analog-stereo`，不會回退至其他來源。部署前必須確認該端點存在：

```bash
# 列出所有 PipeWire 音訊來源
pactl list short sources

# 確認固定來源存在且狀態為 RUNNING 或 IDLE
pactl list sources | grep -A2 'alsa_input.pci-0000_00_1b.0.analog-stereo'
```

若缺少該來源，請先完成音訊硬體驅動與 PipeWire 設定，再繼續部署。

### 4.3 systemd 使用者單元部署

#### 4.3.1 複製單元檔案

```bash
mkdir -p ~/.config/systemd/user
cp deploy/systemd/wifimic-server.service ~/.config/systemd/user/
```

#### 4.3.2 啟用使用者殘留

使 systemd 在使用者登出後仍保持使用者管理器運行，PipeWire 才能持續提供捕獲裝置：

```bash
loginctl enable-linger daniel
```

驗證：

```bash
loginctl show-user daniel --property=Linger
# 輸出應為 Linger=yes
```

#### 4.3.3 重載並啟用服務

```bash
systemctl --user daemon-reload
systemctl --user enable --now wifimic-server
```

#### 4.3.4 確認服務狀態

```bash
systemctl --user status wifimic-server
journalctl --user -u wifimic-server -f
```

預期輸出顯示 `Active: active (running)`，且 journal 無 `203/EXEC` 錯誤（表示二進位檔路徑正確）。

### 4.4 防火牆規則部署

部署腳本 `deploy/linux/wifimic-server-firewall.sh` 會依**目前作用中防火牆服務**自動選擇後端，規則為：

- **允許**：UDP 6902 來自 `192.168.0.200`
- **拒絕**：UDP 6902 來自任何其他來源（port-scoped deny）

#### 4.4.1 執行部署腳本（需 root）

```bash
sudo bash deploy/linux/wifimic-server-firewall.sh
```

#### 4.4.2 後端選擇邏輯（腳本內建）

| 情況 | 行為 |
|------|------|
| `iptables.service` 單獨作用中 | 套用 `wifimic-server-iptables.sh` 規則，持久化至 `/etc/iptables/iptables.rules`，重啟 `iptables.service` |
| `nftables.service` 單獨作用中 | 套用 `wifimic-server.nft` 規則集 |
| `ufw.service` 作用中（且無 iptables/nftables 服務） | 使用 `ufw` 插入 peer allow 規則於第 1 優先序，附加 port-scoped deny 規則，執行 `ufw reload` |
| `ufw.service` 與 iptables/nftables 服務**同時**作用中 | **中止**，拒絕建立第二條封包過濾路徑 |
| `iptables.service` 與 `nftables.service` **同時**作用中 | **中止**，拒絕猜測或變更防火牆狀態 |
| 無支援防火牆服務作用中 | 回退安裝 `nftables`，套用 nft 規則並啟用 `nftables.service` |

**持久化要求**：三種後端的規則都必須在重開機後存活——UFW 規則由 `ufw` 自行持久化（寫入 `/etc/ufw/user.rules`）；iptables 情境由腳本以 `iptables-save` 寫入 `/etc/iptables/iptables.rules` 並重啟 `iptables.service` 載入；nftables 情境由 `nftables.service` 開機時載入規則集。請勿手動另啟第二套防火牆服務，避免雙重過濾路徑。

#### 4.4.3 驗證規則生效

**UFW 情況（arch-daniel 實測環境）**：

```bash
ufw status numbered
# 應包含以下兩條規則（實際編號依主機既有規則而定，allow 規則會被插入在第 1 優先序）：
# 6902/udp  ALLOW IN  192.168.0.200   （註解：wifimic-server peer）
# 6902/udp  DENY IN   Anywhere        （註解：wifimic-server default drop）
```

**nftables 情況**：

```bash
nft list chain inet wifimic_server input
# 應包含：
# ip saddr 192.168.0.200 udp dport 6902 counter accept comment "wifimic-server peer"
# udp dport 6902 counter drop comment "wifimic-server default drop"
```

**iptables 情況**：

```bash
iptables -C INPUT -p udp -s 192.168.0.200 --dport 6902 -j ACCEPT
iptables -C INPUT -p udp --dport 6902 -j DROP
# 兩條指令皆回傳 0 表示規則存在
```

### 4.5 健康檢查

#### 4.5.1 服務與捕獲管線

```bash
# 服務狀態
systemctl --user is-active wifimic-server
# 應輸出 active

# 即時 journal 觀察捕獲啟動
journalctl --user -u wifimic-server -f
# 應見到 parec 啟動、無 EndpointNotFound 錯誤
```

#### 4.5.2 網路連通性（從 Windows 對等端）

> 注意：`Test-NetConnection` 僅支援 TCP，無法測試 UDP。請改用下列 .NET `UdpClient` 方式送出測試資料包：

```powershell
# 從 Windows 對等端送出一個測試資料包（UDP 無連線，伺服器不會回覆）
$udp = New-Object System.Net.Sockets.UdpClient
$udp.Connect('192.168.0.210', 6902)
[void]$udp.Send([byte[]]@(0x01, 0x01), 2)
$udp.Close()
```

UDP 測試無法從 Windows 端單獨確認送達；請接著以第 4.5.3 節的防火牆計數器在 Linux 主機上確認封包確實抵達並被允許規則接受。

#### 4.5.3 防火牆計數器驗證

發送測試封包後，檢查對應計數器是否遞增：

```bash
# UFW
ufw status numbered | grep 6902

# nftables
nft list chain inet wifimic_server input

# iptables
iptables -v -n -L INPUT | grep 6902
```

---

## 5. Windows 用戶端安裝（首次）

安裝腳本 `deploy/windows/install-wifimic-client.ps1` 為冪等、可回滾、支援三種模式：

| 模式 | 參數 | 行為 |
|------|------|------|
| **DryRun** | `-DryRun` | 僅驗證端點存在、路徑可寫、防火牆規則簽章符合，**不寫入任何系統狀態** |
| **TestMode** | `-TestMode` | 在隔離暫存目錄下模擬完整安裝流程（任務、防火牆、檔案複製），**不觸碰真實系統**，輸出 JSON 含 `FakeEvents` 事件流 |
| **Native** | （預設，需 `-AcceptHostMutation`） | 真實寫入 `C:\Program Files\wifimic-client`、註冊排程工作、建立防火牆規則，**需系統管理員互動式工作階段** |

### 5.1 先決條件檢查（僅讀）

```powershell
# 確認 VB-CABLE Input 端點存在
Get-PnpDevice -Class AudioEndpoint -Status OK | Where-Object { $_.FriendlyName -eq 'CABLE Input (VB-Audio Virtual Cable)' }

# 確認建置產物存在
ls C:\src\wifimic\target\release\wifimic_client.exe
```

### 5.2 DryRun 驗證（強烈建議先執行）

```powershell
cd C:\src\wifimic
.\deploy\windows\install-wifimic-client.ps1 `
    -ClientExecutable 'C:\src\wifimic\target\release\wifimic_client.exe' `
    -RenderEndpoint 'CABLE Input (VB-Audio Virtual Cable)' `
    -DryRun
```

預期輸出（JSON）：

```json
{
  "Status": "Validated",
  "Mode": "DryRun",
  "InstallRoot": "C:\\Program Files\\wifimic-client",
  "TaskPath": "\\wifimic\\wifimic-client",
  "FirewallDisplayName": "wifimic-client",
  "RemoteAddress": "192.168.0.210/32",
  "Protocol": "UDP",
  "Port": "6902",
  "Endpoint": "CABLE Input (VB-Audio Virtual Cable)",
  "LogonTrigger": "LogonTrigger",
  "LogonType": "InteractiveToken"
}
```

> **關鍵檢查點**：`Endpoint` 必須完全等於 `CABLE Input (VB-Audio Virtual Cable)`，大小寫、空格皆需一致；若列舉不到該端點，安裝會以 `EndpointNotFound` 失敗。

### 5.3 TestMode 隔離測試（可選）

```powershell
.\deploy\windows\install-wifimic-client.ps1 `
    -ClientExecutable 'C:\src\wifimic\target\release\wifimic_client.exe' `
    -RenderEndpoint 'CABLE Input (VB-Audio Virtual Cable)' `
    -TestMode
```

輸出 JSON 含 `FakeEvents` 陣列，可檢查 `SetTask`、`SetFirewall`、`CopyFile` 等操作順序；暫存目錄於結束時自動清理。

### 5.4 原生安裝（需授權）

> **前置條件**：以**系統管理員身分**開啟 PowerShell，且為**互動式工作階段**（非遠端、非排程、非服務帳號）。

```powershell
cd C:\src\wifimic
.\deploy\windows\install-wifimic-client.ps1 `
    -ClientExecutable 'C:\src\wifimic\target\release\wifimic_client.exe' `
    -RenderEndpoint 'CABLE Input (VB-Audio Virtual Cable)' `
    -AcceptHostMutation
```

成功輸出（JSON）：

```json
{
  "Status": "Installed",
  "Mode": "Native",
  "InstallRoot": "C:\\Program Files\\wifimic-client",
  "ExecutablePath": "C:\\Program Files\\wifimic-client\\wifimic_client.exe",
  "TaskFolder": "\\wifimic\\",
  "TaskName": "wifimic-client",
  "TaskPath": "\\wifimic\\wifimic-client",
  "FirewallDisplayName": "wifimic-client",
  "RemoteAddress": "192.168.0.210/32",
  "Protocol": "UDP",
  "Port": "6902",
  "Endpoint": "CABLE Input (VB-Audio Virtual Cable)",
  "LogonTrigger": "LogonTrigger",
  "LogonType": "InteractiveToken"
}
```

### 5.5 安裝後驗證

```powershell
# 1. 排程工作存在且啟用
Get-ScheduledTask -TaskPath '\wifimic\' -TaskName 'wifimic-client' | Select-Object TaskPath, TaskName, State, Settings

# 2. 執行檔存在且為正確版本
ls 'C:\Program Files\wifimic-client\wifimic_client.exe'

# 3. 防火牆規則存在且簽章符合
Get-NetFirewallRule -DisplayName 'wifimic-client' | Get-NetFirewallPortFilter
Get-NetFirewallRule -DisplayName 'wifimic-client' | Get-NetFirewallAddressFilter

# 4. 端點仍可列舉
Get-PnpDevice -Class AudioEndpoint -Status OK | Where-Object { $_.FriendlyName -eq 'CABLE Input (VB-Audio Virtual Cable)' }
```

> **注意**：安裝腳本會在失敗時自動回滾（移除新建任務、防火牆規則、複製的執行檔、暫存目錄），並驗證先前狀態完全還原。若安裝前已存在同名任務/規則但簽章不符，腳本會拒絕安裝並報 `ConflictingTask`/`ConflictingFirewall`。

---

## 6. Linux 伺服器更新

更新腳本 `deploy/linux/update-wifimic-server.sh` 採用**單一明確標籤/提交**、**乾淨工作區**、**git worktree 隔離建置**、**原子替換**、**強制對等端控制平面煙霧測試**、**自動回滾**機制。

### 6.1 先決條件

- 來源倉庫為乾淨狀態（無未追蹤/修改檔案）
- Linux 已安裝 `ssh`、`base64`，且 `deploy/linux/wifimic-control-smoke-helper.sh` 可執行
- Linux 對 Windows `daniel@192.168.0.200` 已完成反向 SSH 信任：使用金鑰登入可在 `BatchMode=yes` 下成功，並已將 Windows 主機金鑰加入 `known_hosts`；不可依賴密碼或 agent forwarding
- 若 `daniel` 屬於 Windows Administrators，Linux 公鑰必須放在 `C:\ProgramData\ssh\administrators_authorized_keys`，並符合 OpenSSH 對該檔案的 ACL 要求
- Windows 對等端已在 `C:\Users\Daniel\Documents\opencode\wifimic` 建置煙霧測試執行檔：`cargo build --release --bin wifimic_control_smoke`；預設路徑為 `C:\Users\Daniel\Documents\opencode\wifimic\target\release\wifimic_control_smoke.exe`
- `WIFIMIC_CONTROL_SMOKE_HELPER` 環境變數指向**可執行的絕對路徑**，該輔助程式必須能從 **Windows 對等端 (192.168.0.200)** 發起完整的 Start/Heartbeat/Stop Ack 交換，並輸出遠端執行檔的 `wifimic-control-smoke: PASS`
- 如使用其他穩定的絕對 Windows 執行檔路徑，設定 `WIFIMIC_WINDOWS_SMOKE_EXE` 覆寫預設值
- 使用者服務 `wifimic-server` 目前為 `active`
- 現有二進位檔 `~/.local/bin/wifimic_server` 存在且可執行
- 現有使用者單元 `~/.config/systemd/user/wifimic-server.service` 存在

> **關鍵限制**：伺服器**拒絕所有非 192.168.0.200 來源的資料包**，因此 localhost 健康檢查**永遠失敗**；必須提供真實的對等端輔助程式。

### 6.2 執行更新

```bash
cd ~/src/wifimic
chmod +x ./deploy/linux/wifimic-control-smoke-helper.sh
WIFIMIC_CONTROL_SMOKE_HELPER="$PWD/deploy/linux/wifimic-control-smoke-helper.sh" \
./deploy/linux/update-wifimic-server.sh v1.2.3
```

或指定提交雜湊：

```bash
WIFIMIC_CONTROL_SMOKE_HELPER="$PWD/deploy/linux/wifimic-control-smoke-helper.sh" \
./deploy/linux/update-wifimic-server.sh a1b2c3d4e5f6
```

若煙霧測試執行檔不在預設路徑，可同時指定另一個穩定的絕對 Windows 路徑：

```bash
WIFIMIC_WINDOWS_SMOKE_EXE='D:\wifimic\target\release\wifimic_control_smoke.exe' \
WIFIMIC_CONTROL_SMOKE_HELPER="$PWD/deploy/linux/wifimic-control-smoke-helper.sh" \
./deploy/linux/update-wifimic-server.sh v1.2.3
```

### 6.3 更新流程內部步驟（供審計參考）

1. 驗證修訂格式、正整數逾時、絕對路徑、輔助程式可執行
2. 檢查工作區乾淨度；髒則中止
3. `git fetch` 指定標籤/提交（必要時）
4. 解析為單一 40 字元提交雜湊
5. 建立交易目錄，備份先前二進位檔、單元檔、SHA256、file 輸出
6. `git worktree add --detach` 至暫存目錄
7. `cargo build --release --bin wifimic_server` 在 worktree 內建置
8. 驗證產物為 ELF 執行檔、SHA256 計算、修訂一致
9. **MUTATION_STARTED=1**：停止使用者服務、原子替換二進位檔、重啟服務
10. 等待服務變 `active`（預設 45 秒）
11. 執行 `WIFIMIC_CONTROL_SMOKE_HELPER 192.168.0.210 6902`，驗證完整 Ack 交換
12. 成功：輸出新/舊 SHA256、修訂；失敗：EXIT trap 觸發自動回滾

### 6.4 自動回滾行為

若任一步驟失敗（建置、服務啟動、煙霧測試、逾時），`on_exit` trap 會：

1. 還原先前二進位檔（`atomic_replace`）
2. 還原先前單元檔並 `systemctl --user daemon-reload`
3. 重啟服務並等待 `active`
4. 清理 worktree、交易目錄
5. 輸出 `wifimic server rollback restored an active service` 或 `wifimic server rollback could not prove an active service`

### 6.5 更新後驗證

```bash
# 服務狀態
systemctl --user is-active wifimic-server
# 應輸出 active

# 二進位檔版本確認
file ~/.local/bin/wifimic_server
sha256sum ~/.local/bin/wifimic_server

# Journal 觀察
journalctl --user -u wifimic-server -f
```

---

## 7. Windows 用戶端更新

更新腳本 `deploy/windows/update-wifimic-client.ps1` 採用**單一明確標籤/提交**、**乾淨工作區**、**先抓取再解析**、**git worktree 隔離建置**、**停用任務→原子替換→還原任務→啟動任務→健康檢查**、**自動回滾**機制。

### 7.1 先決條件

- 來源倉庫為乾淨狀態（`git status --porcelain --untracked-files=all` 為空）
- 規範排程工作 `\wifimic\wifimic-client` **存在且啟用**，狀態為 `Ready` 或 `Running`
- 已安裝執行檔 `C:\Program Files\wifimic-client\wifimic_client.exe` 存在
- `CABLE Input (VB-Audio Virtual Cable)` 端點可列舉
- 原生模式需：**系統管理員互動式工作階段** + `-AcceptHostMutation`

### 7.2 執行更新

```powershell
cd C:\src\wifimic
.\deploy\windows\update-wifimic-client.ps1 -Tag v1.2.3 -AcceptHostMutation
```

或指定提交雜湊：

```powershell
.\deploy\windows\update-wifimic-client.ps1 -Tag a1b2c3d4e5f6 -AcceptHostMutation
```

### 7.3 測試模式（隔離驗證）

```powershell
.\deploy\windows\update-wifimic-client.ps1 -Tag v1.2.3 -TestMode
```

輸出 JSON 含 `FakeEvents`、`PriorExecutableRestored`（回滾驗證）、`TaskXmlPreserved` 等欄位；暫存目錄自動清理。

> **FailurePoint 測試**：`-FailurePoint` 參數可注入確定性失敗點（`DirtyCheckout`、`BadRevision`、`AmbiguousRevision`、`Fetch`、`Worktree`、`Build`、`Endpoint`、`DisableTask`、`StopTask`、`BeforeAtomicSwap`、`AtomicSwap`、`AfterAtomicSwap`、`SetTask`、`TaskRegistration`、`StartTask`、`Health`），僅限 `-TestMode` 使用。

### 7.4 更新流程內部步驟（供審計參考）

1. 驗證修訂文字格式（非空、無前後空白、非選項開頭、無空白/NUL、無 Git 複合語法）
2. `GetSourceStatus`：檢查工作區乾淨度
3. `FetchTags`：`git fetch --tags --prune origin`
4. `ResolveRevision`：`git rev-parse --verify --end-of-options <tag>^{commit}` → 單一 40 字元 SHA
5. 捕獲先前任務 XML、啟用狀態、執行檔位元組/SHA256
6. 前置檢查：任務啟用、狀態可用、執行檔存在、端點可列舉
7. 建立暫存目錄（`.wifimic-client-stage-<guid>`）、交易目錄（`.wifimic-client-transaction-<guid>`）
8. `git worktree add --detach` 至暫存目錄
9. `cargo build --release --locked -p wifimic_client` 在 worktree 內建置
10. 捕獲候選執行檔 SHA256，驗證非空
11. 再次前置檢查（建置期間未被竄改）
12. 寫入交易位元組（先前執行檔備份）
13. **任務變更開始**：停用任務 → 若 Running 則停止任務並等待非 Running
14. **二進位變更開始**：同磁碟區原子替換（`File.Replace` / `File.Move`）
15. 還原任務 XML（先前版本）、啟用任務、啟動任務
16. 健康檢查輪詢：任務啟用、狀態 Ready/Running、端點可列舉（若先前 Running 則需 Running）
17. 成功：輸出新/舊 SHA256、修訂、任務狀態；失敗：自動回滾

### 7.5 自動回滾行為

若任務/二進位變更已開始且後續步驟失敗：

1. 停止目前任務（若 Running 先 Stop 並等待）
2. 原子還原先前執行檔位元組
3. 還原先前任務 XML、啟用任務、若先前 Running 則啟動
4. 驗證：執行檔 SHA256 還原、任務 XML 完全一致、啟用狀態、狀態 Ready/Running、若先前 Running 則為 Running
5. 清理 worktree、暫存、交易目錄
6. 輸出 `RollbackFailed` 或成功回滾訊息

### 7.6 更新後驗證

```powershell
# 1. 排程工作存在、啟用、狀態 Ready/Running
Get-ScheduledTask -TaskPath '\wifimic\' -TaskName 'wifimic-client' | Select-Object TaskPath, TaskName, State, Settings

# 2. 執行檔 SHA256 已變更
$bytes = [System.IO.File]::ReadAllBytes('C:\Program Files\wifimic-client\wifimic_client.exe')
$sha = [System.Security.Cryptography.SHA256]::Create()
[BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant()

# 3. 端點仍可列舉
Get-PnpDevice -Class AudioEndpoint -Status OK | Where-Object { $_.FriendlyName -eq 'CABLE Input (VB-Audio Virtual Cable)' }

# 4. 即時觀察（若有 journal 等效機制）
# 目前以排程工作狀態與端點存在為主要可觀測指標
```

---

## 8. 端到端驗證流程

部署/更新完成後，執行下列有序驗證確認雙向通道正常：

### 8.1 Linux 端：服務與防火牆

```bash
# 服務啟用中
systemctl --user is-active wifimic-server
# → active

# Journal 無錯誤
journalctl --user -u wifimic-server --since '5 minutes ago' | grep -i error
# → 無輸出

# 防火牆計數器（UFW 例）
ufw status numbered | grep 6902
# → 允許規則計數器應有遞增（若有測試流量）
```

### 8.2 Windows 端：任務、防火牆、端點

```powershell
# 排程工作
$task = Get-ScheduledTask -TaskPath '\wifimic\' -TaskName 'wifimic-client'
$task.State -in @('Ready', 'Running') -and $task.Settings.Enabled
# → True

# 防火牆規則
$rule = Get-NetFirewallRule -DisplayName 'wifimic-client'
$port = Get-NetFirewallPortFilter -AssociatedNetFirewallRule $rule
$addr = Get-NetFirewallAddressFilter -AssociatedNetFirewallRule $rule
$rule.Enabled -eq 'True' -and $port.Protocol -eq 'UDP' -and $port.LocalPort -eq '6902' -and $addr.RemoteAddress -eq '192.168.0.210'
# → True

# 端點
Get-PnpDevice -Class AudioEndpoint -Status OK | Where-Object { $_.FriendlyName -eq 'CABLE Input (VB-Audio Virtual Cable)' }
# → 存在
```

### 8.3 實際音訊流驗證（人工）

1. Linux 端：`journalctl --user -u wifimic-server -f` 觀察 `parec` 讀取與 UDP 發送
2. Windows 端：登入/重新登入觸發排程工作，或手動 `schtasks /Run /TN '\wifimic\wifimic-client'`
3. 確認 VB-CABLE Output 端有聲音輸出（需實體監聽或錄音軟體）
4. Linux 端 journal 應見到控制平面 Start Ack、Heartbeat Ack 交換

---

## 9. 疑難排解

> **診斷順序原則**：**先排查 Windows 端（用戶端），再排查 Linux 端（伺服器）**。Windows 端無法連線時，最常見根因為伺服器未啟動、防火牆阻擋、或對等端 IP 不符——這些在 Windows 端表現為「伺服器不可達」。

### 9.1 Windows 端常見失敗

| 現象 | 可能原因 | 排查步驟 |
|------|----------|----------|
| 安裝/更新報 `EndpointNotFound` | VB-CABLE 未安裝、端點名稱不符、音訊服務未啟動 | 1. `Get-PnpDevice -Class AudioEndpoint -Status OK` 確認 `CABLE Input (VB-Audio Virtual Cable)` 存在<br>2. 重新安裝 VB-CABLE 驅動<br>3. 重啟 Windows Audio 服務 `Restart-Service Audiosrv` |
| 安裝/更新報 `AdministratorRequired` / `InteractiveSessionRequired` | 權限不足或非互動式工作階段 | 以「系統管理員身分執行」開啟 PowerShell，確認 `$([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)` 為 `True` |
| 安裝/更新報 `ConflictingTask` / `ConflictingFirewall` | 既有同名任務/規則但簽章不符（非本專案擁有） | 1. `Get-ScheduledTask -TaskPath '\wifimic\' -TaskName 'wifimic-client'` 檢查現有任務<br>2. `Get-NetFirewallRule -DisplayName 'wifimic-client'` 檢查現有規則<br>3. 手動移除衝突物件後重試 |
| 排程工作狀態非 `Ready`/`Running` | 任務被停用、執行檔遺失、登入觸發未生效 | 1. `Get-ScheduledTask ...` 檢查 `State`、`Settings.Enabled`<br>2. 確認 `C:\Program Files\wifimic-client\wifimic_client.exe` 存在<br>3. 手動 `schtasks /Run /TN '\wifimic\wifimic-client'` 觀察啟動 |
| 防火牆規則缺失或 RemoteAddress 不為 `192.168.0.210/32` | 安裝未完成、規則被手動刪除、群組原則覆寫 | 1. `Get-NetFirewallRule -DisplayName 'wifimic-client'` 確認存在<br>2. `Get-NetFirewallAddressFilter ...` 確認 `RemoteAddress`<br>3. 重跑安裝/更新腳本（會自動修正簽章不符） |
| 更新報 `DirtyCheckout` | 來源倉庫有未提交變更 | `git status --porcelain --untracked-files=all` 確認輸出為空；提交或暫存變更後重試 |
| 更新報 `AmbiguousRevision` / `BadRevision` | 標籤/提交不存在、未抓取遠端、語法錯誤 | 1. 確認標籤名稱拼寫 `git tag -l`<br>2. `git fetch --tags --prune origin`<br>3. 修訂必須為單一標籤名或 7-64 字元十六進位提交雜湊 |
| 更新報 `CrossVolumeSwap` | 暫存目錄與安裝目錄不在同一磁碟區 | 確保 `C:\src\wifimic` 與 `C:\Program Files\wifimic-client` 同屬 C: 槽；若不同，需手動調整暫存路徑邏輯 |

### 9.2 Linux 端常見失敗

| 現象 | 可能原因 | 排查步驟 |
|------|----------|----------|
| `systemctl --user status wifimic-server` 顯示 `203/EXEC` | 二進位檔路徑錯誤或未安裝 | 確認 `~/.local/bin/wifimic_server` 存在且可執行；重跑第 4.1 節安裝步驟 |
| 服務啟動但 journal 出現 `EndpointNotFound` | 固定 PipeWire 來源不存在 | 執行第 4.2 節驗證指令；檢查 `pipewire`、`wireplumber` 服務狀態 |
| `loginctl enable-linger` 後服務仍在登出時停止 | linger 未生效 | `loginctl show-user daniel --property=Linger` 確認為 `yes`；重啟使用者管理器 `systemctl --user daemon-reload` |
| 防火牆腳本中止並顯示「both active」 | 多重防火牆服務同時啟用 | `systemctl list-units 'iptables.service' 'nftables.service' 'ufw.service' --state=active` 確認僅有一個作用中；停用多餘服務 |
| UFW 規則顯示 `ALLOW IN Anywhere` | 既有規則過寬 | 腳本會偵測並中止；手動 `ufw delete` 該規則後重跑腳本 |
| 更新腳本報 `WIFIMIC_CONTROL_SMOKE_HELPER is required` | 未設定環境變數或路徑無效 | `export WIFIMIC_CONTROL_SMOKE_HELPER=/absolute/path/to/helper`；確認檔案存在且可執行 `ls -l $WIFIMIC_CONTROL_SMOKE_HELPER` |
| 更新腳本報 `peer-originated control-session smoke ... did not prove a complete control-session Ack exchange` | 輔助程式未從 192.168.0.200 發送、伺服器未回 Ack、網路不通 | 1. 確認輔助程式在 Windows 端執行並綁定 192.168.0.200<br>2. Linux 端 `ufw status numbered` 確認允許規則計數器遞增<br>3. Linux 端 `journalctl --user -u wifimic-server -f` 觀察控制平面收到 Start/Heartbeat/Stop |
| 更新腳本報 `source checkout is dirty` | 工作區有未追蹤/修改檔案 | `git status --porcelain --untracked-files=all` 確認輸出為空；提交或 `git stash` 後重試 |

### 9.3 伺服器不可達（Windows 端視角）

**症狀**：Windows 用戶端啟動後，journal 無 Start Ack、Heartbeat Ack，最終進入 `Unreachable` 狀態。

**診斷順序**（Windows 先行）：

1. **Windows 端防火牆**：確認 `wifimic-client` 規則啟用、UDP 6902、RemoteAddress `192.168.0.210/32`
2. **Windows 端排程工作**：確認任務啟用、執行檔存在、狀態 `Running`
3. **Windows 端端點**：確認 `CABLE Input (VB-Audio Virtual Cable)` 可列舉
4. **網路基本連通**：`ping 192.168.0.210` 確認 L3 連通
5. **Linux 端服務**：`systemctl --user is-active wifimic-server` → `active`
6. **Linux 端防火牆**：`ufw status numbered | grep 6902` 確認允許規則存在、計數器遞增
7. **Linux 端捕獲源**：`pactl list sources | grep -A2 'alsa_input.pci-0000_00_1b.0.analog-stereo'` 確認 RUNNING/IDLE
8. **Linux 端 journal**：`journalctl --user -u wifimic-server -f` 觀察是否收到 Start 封包、是否發送 Ack

> **關鍵洞察**：若 Windows 端一切正常但 Linux 端防火牆計數器無遞增，問題在 Windows 發送路徑（防火牆輸出規則、路由、IP 衝突）；若計數器有遞增但 journal 無 Start 收到，問題在 Linux 服務未綁定/未監聽/捕獲管線故障。

### 9.4 回滾驗證失敗

| 腳本 | 現象 | 排查 |
|------|------|------|
| Linux 更新 | `wifimic server rollback could not prove an active service` | 1. 手動 `systemctl --user start wifimic-server`<br>2. 檢查 journal 是否有 `203/EXEC` 或 `EndpointNotFound`<br>3. 確認先前備份二進位檔 `~/.local/bin/wifimic_server` 完整 |
| Windows 更新 | `RollbackFailed: The prior executable hash was not restored` / `The prior task XML was not restored with the task enabled` | 1. 以系統管理員 PowerShell 手動還原：`schtasks /Change /TN '\wifimic\wifimic-client' /DISABLE` → 複製備份 exe → `schtasks /Create /TN '\wifimic\wifimic-client' /XML <backup.xml> /F` → `/ENABLE`<br>2. 檢查交易目錄 `.wifimic-client-transaction-*/prior-client.exe` 是否存在 |

---

## 10. 完整移除部署

### 10.1 Linux 伺服器端

```bash
# 停用並移除 systemd 使用者單元
systemctl --user disable --now wifimic-server
rm ~/.config/systemd/user/wifimic-server.service
systemctl --user daemon-reload

# 移除二進位檔
rm ~/.local/bin/wifimic_server

# 移除防火牆規則（依對應後端執行）
# UFW：
ufw delete <allow-rule-number>
ufw delete <deny-rule-number>
ufw reload

# nftables：
sudo nft delete table inet wifimic_server
# 若為回退安裝的 nftables.service，可選擇停用
# sudo systemctl disable --now nftables.service

# iptables：
sudo iptables -D INPUT -p udp -s 192.168.0.200 --dport 6902 -j ACCEPT
sudo iptables -D INPUT -p udp --dport 6902 -j DROP
sudo iptables-save | sudo tee /etc/iptables/iptables.rules >/dev/null
sudo systemctl restart iptables.service

# 取消使用者殘留（若無其他服務需要）
loginctl disable-linger daniel
```

### 10.2 Windows 用戶端端

```powershell
# 以系統管理員身分執行

# 移除排程工作
schtasks /Delete /TN '\wifimic\wifimic-client' /F

# 移除防火牆規則
Remove-NetFirewallRule -DisplayName 'wifimic-client'

# 移除安裝目錄
Remove-Item -LiteralPath 'C:\Program Files\wifimic-client' -Recurse -Force
```

---

## 11. 限制與已知問題

1. **無第三方 LAN 來源驗證**：部署環境僅有 `192.168.0.200` 一個對等端，port-scoped deny 計數器未經實測遞增；規則邏輯經程式碼審查確認正確。
2. **非乾淨主機狀態**：`arch-daniel` 已有既有套件與服務狀態，本指南未涵蓋全新安裝情境；若在乾淨主機部署，請先安裝 `pipewire`、`wireplumber`、`rustup` 等基礎套件。
3. **Rust LSP 逾時**：工作區 Rust LSP 會逾時，不影響 `cargo build`、`cargo test`、`cargo clippy` 正常運作。
4. **認證路徑禁止**：本指南不包含任何認證相關路徑；請勿將憑證寫入持久化檔案。
5. **即時更新需人工授權**：Linux 更新需對等端輔助程式、Windows 更新需系統管理員互動式工作階段；無背景自動更新機制。
6. **VB-CABLE 方向固定**：僅支援 `CABLE Input (VB-Audio Virtual Cable)` 作為渲染端點；`CABLE Output` 不在支援範圍內。
7. **對等端 IP 硬編碼**：雙端皆將對方 IP 寫死於程式碼/腳本中；變更 IP 需修改原始碼重建。

---

## 12. 參考檔案

- `docs/release-process.md` — **每版必須遵循的發布與真實部署驗證強制流程**（推送、打標籤、確認 GitHub Release、實際部署、端到端人工驗證）
- `docs/deployment-linux.md` — Linux 伺服器詳細部署指南（本文件第 4 節為其摘要）
- `deploy/systemd/wifimic-server.service` — systemd 使用者單元定義
- `deploy/linux/wifimic-server-firewall.sh` — 防火牆部署主腳本（含後端選擇邏輯）
- `deploy/linux/wifimic-server-iptables.sh` — iptables 規則定義
- `deploy/linux/wifimic-server.nft` — nftables 規則集
- `deploy/linux/update-wifimic-server.sh` — Linux 伺服器更新腳本
- `deploy/windows/install-wifimic-client.ps1` — Windows 用戶端安裝腳本
- `deploy/windows/update-wifimic-client.ps1` — Windows 用戶端更新腳本
- `apps/wifimic_server/src/capture_types.rs` — 固定捕獲源與 `parec` 參數
- `apps/wifimic_server/src/network.rs` — UDP 連接埠、對等端 IP、來源 IP 驗證邏輯
- `apps/wifimic_client/src/lib.rs` — Windows 端點列舉、渲染管線

---

## 13. 版本與驗證記錄

*文件版本：以 git 歷史為準（於倉庫根目錄執行 `git log -1 -- docs/deployment.md` 可查得本檔案最後修訂提交）*
*部署驗證日期：2026-08-22*
*對應任務：Todo 18（Wave 4 部署文件整合）*
