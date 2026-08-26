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
| Windows 安裝根目錄 | `C:\Program Files\wifimic-client` | `apps/wifimic_client/src/installer.rs:12` |
| Windows 執行檔名稱 | `wifimic_client.exe` | `apps/wifimic_client/src/installer.rs:14` |
| Windows 排程工作資料夾 | `\wifimic\` | `apps/wifimic_client/src/installer.rs:16` |
| Windows 排程工作名稱 | `wifimic-client` | `apps/wifimic_client/src/installer.rs:16` |
| Windows 排程工作完整路徑 | `\wifimic\wifimic-client` | `apps/wifimic_client/src/installer.rs:16` |
| Windows 防火牆顯示名稱 | `wifimic-client` | `apps/wifimic_client/src/installer.rs:18` |
| Windows 防火牆遠端位址 | `192.168.0.210/32` | `apps/wifimic_client/src/installer.rs:20` |
| Windows 防火牆通訊埠 | `6902` (UDP) | `apps/wifimic_client/src/installer.rs:22` |
| Windows 固定渲染端點 | `CABLE Input (VB-Audio Virtual Cable)` | `apps/wifimic_client/src/installer.rs:24` |
| Linux 服務名稱 | `wifimic-server` | `deploy/systemd/wifimic-server.service` |
| Linux 二進位檔路徑 | `~/.local/bin/wifimic_server` | systemd 單元 `ExecStart` |
| Linux systemd 使用者單元路徑 | `~/.config/systemd/user/wifimic-server.service` | `deploy/linux/update-wifimic-server.sh:7` |
| Linux 更新腳本 | `deploy/linux/update-wifimic-server.sh` | — |
| Windows 安裝器 | `wifimic_client_installer.exe install` | `apps/wifimic_client/src/bin/wifimic_client_installer.rs` |
| Windows 更新機制 | `wifimic_client.exe upgrade` | `apps/wifimic_client/src/client_update.rs` |

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

Windows 首次安裝由公開 bootstrap 腳本下載並驗證 release zip，再直接呼叫編譯後的 `wifimic_client_installer.exe`。終端使用者通常不需要直接呼叫安裝器；以下命令說明其底層機制。

### 5.1 先決條件檢查（僅讀）

```powershell
# 確認 VB-CABLE Input 端點存在
Get-PnpDevice -Class AudioEndpoint -Status OK | Where-Object { $_.FriendlyName -eq 'CABLE Input (VB-Audio Virtual Cable)' }

# 確認 release 成品存在
ls C:\src\wifimic\target\release\wifimic_client.exe
ls C:\src\wifimic\target\release\wifimic_client_installer.exe
```

### 5.2 公開 bootstrap 安裝（建議）

```powershell
irm https://github.com/danielrepublic/wifimic/releases/latest/download/install-wifimic-windows.ps1 | iex
```

bootstrap 會下載 `wifimic-windows-x86_64.zip` 與其 `.sha256` manifest，驗證 archive 的 SHA-256，解壓出 `wifimic_client.exe` 與 `wifimic_client_installer.exe`，再以底下 5.3 的 `install` 命令完成主機變更。它必須在系統管理員互動式工作階段中執行。

### 5.3 原生安裝器呼叫（底層機制）

```powershell
cd C:\src\wifimic
$client = 'C:\src\wifimic\target\release\wifimic_client.exe'
$installer = 'C:\src\wifimic\target\release\wifimic_client_installer.exe'
& $installer install --client-executable $client --render-endpoint 'CABLE Input (VB-Audio Virtual Cable)' --accept-host-mutation
```

`--accept-host-mutation` 是明確授權主機變更的必要旗標；安裝器也會自行檢查系統管理員權限與互動式工作階段。一般使用者應使用 5.2 的 bootstrap，而不是直接執行此底層命令。

### 5.4 原生安裝流程與回滾

安裝器在寫入前先捕獲既有排程工作、防火牆規則與用戶端執行檔，接著依序：

1. 將候選 `wifimic_client.exe` 複製到私有暫存位置並確認檔案非空
2. 原子替換安裝根目錄中的執行檔
3. 註冊 canonical `InteractiveToken` 登入觸發排程工作
4. 建立 canonical UDP 6902 防火牆規則，僅允許 `192.168.0.210/32`
5. 啟用並啟動排程工作，等待排程工作與 `CABLE Input (VB-Audio Virtual Cable)` 健康

任何寫入後步驟失敗，安裝器會回復已捕獲的排程工作、防火牆規則與執行檔；原先不存在的項目則移除。若回復本身失敗，會以不同的失敗結果回報，而不是假定安裝成功。

安裝器結束碼為：`0` 成功、`10` 主機變更被拒絕（包含權限/工作階段或未明確接受）、`11` 命令列參數或端點不合法、`20` 主機變更失敗、`21` 回復失敗。

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

> **注意**：編譯後安裝器會在任何寫入後失敗時自動回滾，並在回復失敗時以退出碼 `21` 明確回報；它不是背景服務，也不會在沒有 `--accept-host-mutation` 時寫入主機狀態。

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

Windows 更新的主要入口是已安裝的 `wifimic_client.exe`：使用者可用命令列檢查/更新，或從系統匣執行「檢查更新…」。這些入口會在內部下載 release、暫存兩個 Windows 執行檔，並提升執行 `wifimic_client_installer.exe upgrade ...`；終端使用者不需要直接呼叫底層安裝器。

### 7.1 先決條件

- 規範排程工作 `\wifimic\wifimic-client` **存在且啟用**，狀態為 `Ready` 或 `Running`
- 已安裝執行檔 `C:\Program Files\wifimic-client\wifimic_client.exe` 存在
- `CABLE Input (VB-Audio Virtual Cable)` 端點可列舉
- 可連線至 GitHub release，且 release zip 同時包含 `wifimic_client.exe` 與 `wifimic_client_installer.exe`
- 更新需在**系統管理員互動式工作階段**中執行

### 7.2 執行更新

```powershell
# 只檢查，不安裝
& 'C:\Program Files\wifimic-client\wifimic_client.exe' check-update

# 更新至最新版本
& 'C:\Program Files\wifimic-client\wifimic_client.exe' upgrade

# 更新至指定 release tag
& 'C:\Program Files\wifimic-client\wifimic_client.exe' upgrade --tag v1.2.3
```

`check-update` 僅查詢並回報版本，不變更主機；`upgrade` 會下載、驗證並套用更新。不要重新執行首次安裝 one-liner，也不要由使用者直接呼叫底層 `wifimic_client_installer.exe upgrade`。

### 7.3 系統匣更新（使用者確認）

在系統匣選取「檢查更新…」後，用戶端會先查詢最新 release：

1. 若有新版本，顯示確認對話框
2. 使用者確認後，呼叫更新流程並由 Windows UAC 提升執行 bundled installer
3. 若已是最新版本，顯示資訊對話框，不進行主機變更

取消確認不會啟動更新。

### 7.4 更新流程內部步驟（供審計參考）

1. `upgrade` 無 `--tag` 時發現最新 release；指定 `--tag vX.Y.Z` 時驗證 release tag
2. 下載 `wifimic-windows-x86_64.zip` 與 `wifimic-windows-x86_64.zip.sha256`，以 SHA-256 manifest 驗證 archive
3. 在暫存目錄解壓出**兩個**成品：`wifimic_client.exe` 與 `wifimic_client_installer.exe`
4. 以 UAC 提升 bundled installer，執行 `upgrade --client-executable <staged wifimic_client.exe> --accept-host-mutation`
5. installer 先檢查系統管理員權限與互動式工作階段，捕獲既有排程工作、防火牆規則與執行檔
6. 停用並停止排程工作，原子替換用戶端執行檔，還原既有排程工作 XML，啟用並啟動排程工作
7. 等待排程工作與 `CABLE Input (VB-Audio Virtual Cable)` 健康，最長等待 45 秒；完成後清理暫存目錄

### 7.5 自動回滾行為

若任務或執行檔變更已開始且後續步驟失敗：

1. 還原先前執行檔位元組
2. 還原先前排程工作與防火牆狀態；原子更新尚未成功建立的項目則移除
3. 清理更新暫存目錄
4. 回復失敗時，installer 以退出碼 `21` 回報；更新入口會將此結果顯示為更新與回復皆失敗

### 7.6 更新後驗證

```powershell
# 1. 排程工作存在、啟用、狀態 Ready/Running
Get-ScheduledTask -TaskPath '\wifimic\' -TaskName 'wifimic-client' | Select-Object TaskPath, TaskName, State, Settings

# 2. 執行檔存在且回報版本
& 'C:\Program Files\wifimic-client\wifimic_client.exe' --version

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
| 安裝/更新健康檢查失敗 | VB-CABLE 未安裝、端點名稱不符、音訊服務未啟動，或排程工作未恢復健康 | 1. `Get-PnpDevice -Class AudioEndpoint -Status OK` 確認 `CABLE Input (VB-Audio Virtual Cable)` 存在<br>2. `Get-ScheduledTask -TaskPath '\wifimic\' -TaskName 'wifimic-client'` 檢查狀態<br>3. 確認 `C:\Program Files\wifimic-client\wifimic_client.exe` 存在 |
| 安裝器退出碼 `10` | 未在系統管理員互動式工作階段執行，或未提供 `--accept-host-mutation` | 以「系統管理員身分執行」開啟 PowerShell，且確認不是服務、排程或其他非互動式工作階段；底層命令需帶 `--accept-host-mutation` |
| 安裝器退出碼 `11` | 安裝器命令列參數或固定渲染端點不合法 | 依 5.3 的固定 grammar 執行，並確認端點完全等於 `CABLE Input (VB-Audio Virtual Cable)` |
| 安裝器退出碼 `20` | 主機變更期間的操作失敗 | 保留安裝器 stderr 訊息，確認排程工作、`C:\Program Files\wifimic-client` 權限與端點狀態後重試；安裝器會先嘗試自動回復 |
| 安裝器退出碼 `21` | 自動回復也失敗 | 不要重複執行更新；先人工檢查排程工作、執行檔與防火牆狀態，再依 7.6 驗證 |
| 排程工作狀態非 `Ready`/`Running` | 任務被停用、執行檔遺失、登入觸發未生效 | 1. `Get-ScheduledTask ...` 檢查 `State`、`Settings.Enabled`<br>2. 確認 `C:\Program Files\wifimic-client\wifimic_client.exe` 存在<br>3. 手動 `schtasks /Run /TN '\wifimic\wifimic-client'` 觀察啟動 |
| 防火牆規則缺失或 RemoteAddress 不為 `192.168.0.210/32` | 安裝未完成、規則被手動刪除、群組原則覆寫 | 1. `Get-NetFirewallRule -DisplayName 'wifimic-client'` 確認存在<br>2. `Get-NetFirewallAddressFilter ...` 確認 `RemoteAddress`<br>3. 重新執行 bootstrap 安裝或已安裝用戶端的 `upgrade` |
| `wifimic_client check-update` 失敗 | GitHub release 無法連線或版本資訊無法解析 | 確認網路與 GitHub 存取，再重試唯讀檢查 |
| `wifimic_client upgrade --tag` 失敗 | 指定 release tag 不存在或格式不被接受 | 確認 GitHub Release 的 tag 拼寫，再以 `upgrade --tag vX.Y.Z` 重試 |

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
| Windows 安裝/更新 | 安裝器退出碼 `21`，或更新入口顯示回復也失敗 | 1. 暫停重試並以系統管理員 PowerShell 檢查 `Get-ScheduledTask`、`Get-NetFirewallRule` 與安裝目錄<br>2. 確認 `wifimic_client.exe --version`、排程工作狀態與 VB-CABLE 端點，再依第 7.6 節驗證 |

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
- `apps/wifimic_client/src/bin/wifimic_client_installer.rs` — Windows 用戶端安裝器命令列入口
- `apps/wifimic_client/src/installer.rs` — Windows 安裝/更新交易、主機變更與回滾
- `apps/wifimic_client/src/client_update.rs` — Windows release 檢查、下載、驗證、暫存與提升更新
- `apps/wifimic_server/src/capture_types.rs` — 固定捕獲源與 `parec` 參數
- `apps/wifimic_server/src/network.rs` — UDP 連接埠、對等端 IP、來源 IP 驗證邏輯
- `apps/wifimic_client/src/lib.rs` — Windows 端點列舉、渲染管線

---

## 13. 版本與驗證記錄

*文件版本：以 git 歷史為準（於倉庫根目錄執行 `git log -1 -- docs/deployment.md` 可查得本檔案最後修訂提交）*
*部署驗證日期：2026-08-22*
*對應任務：Todo 18（Wave 4 部署文件整合）*
