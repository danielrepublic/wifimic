# wifimic Linux 伺服器部署指南

本文件記錄在 `arch-daniel` (192.168.0.210) 上部署 wifimic 私有伺服器的完整流程，針對 Windows 對等端 `192.168.0.200`、UDP 6902、固定 PipeWire 捕獲源 `alsa_input.pci-0000_00_1b.0.analog-stereo` 的實際成品。

> **重要**：草稿中出現的 `sudo systemctl start wifimic` 僅為概念說明。實際機制為 **使用者層級 systemd 單元** 搭配 `loginctl enable-linger daniel`，使 PipeWire 維持在使用者工作階段中。

---

## 1. 先決條件

- Arch Linux（或衍生發行版）已安裝 `base-devel`、`rustup`、`pipewire`、`wireplumber`、`ufw` 或 `nftables`/`iptables` 其中一套防火牆後端
- 使用者 `daniel` 具備 `sudo` 權限
- 區域網路固定 IP：Linux 主機 `192.168.0.210`、Windows 對等端 `192.168.0.200`
- UDP 6902 僅允許 `192.168.0.200` 存取

---

## 2. 取得私有倉庫原始碼

```bash
# 於任意目錄（建議 ~/src）克隆私有倉庫
git clone git@github.com:<your-account>/wifimic.git
cd wifimic
```

> 若已有本地副本，改用 `git fetch origin && git checkout main && git pull` 更新至最新提交。

---

## 3. 本地 Rust 建置與安裝

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

---

## 4. 固定 PipeWire 捕獲源驗證

伺服器**僅**接受 `alsa_input.pci-0000_00_1b.0.analog-stereo`，不會回退至其他來源。部署前必須確認該端點存在：

```bash
# 列出所有 PipeWire 音訊來源
pactl list short sources

# 確認固定來源存在且狀態為 RUNNING 或 IDLE
pactl list sources | grep -A2 'alsa_input.pci-0000_00_1b.0.analog-stereo'
```

若缺少該來源，請先完成音訊硬體驅動與 PipeWire 設定，再繼續部署。

---

## 5. systemd 使用者單元部署

### 5.1 複製單元檔案

```bash
mkdir -p ~/.config/systemd/user
cp deploy/systemd/wifimic-server.service ~/.config/systemd/user/
```

### 5.2 啟用使用者殘留

使 systemd 在使用者登出後仍保持使用者管理器運行，PipeWire 才能持續提供捕獲裝置：

```bash
loginctl enable-linger daniel
```

驗證：

```bash
loginctl show-user daniel --property=Linger
# 輸出應為 Linger=yes
```

### 5.3 重載並啟用服務

```bash
systemctl --user daemon-reload
systemctl --user enable --now wifimic-server
```

### 5.4 確認服務狀態

```bash
systemctl --user status wifimic-server
journalctl --user -u wifimic-server -f
```

預期輸出顯示 `Active: active (running)`，且 journal 無 `203/EXEC` 錯誤（表示二進位檔路徑正確）。

---

## 6. 防火牆規則部署

部署腳本 `deploy/linux/wifimic-server-firewall.sh` 會依**目前作用中防火牆服務**自動選擇後端，規則為：

- **允許**：UDP 6902 來自 `192.168.0.200`
- **拒絕**：UDP 6902 來自任何其他來源（port-scoped deny）

### 6.1 執行部署腳本（需 root）

```bash
sudo bash deploy/linux/wifimic-server-firewall.sh
```

### 6.2 後端選擇邏輯（腳本內建）

| 情況 | 行為 |
|------|------|
| `iptables.service` 單獨作用中 | 套用 `wifimic-server-iptables.sh` 規則，持久化至 `/etc/iptables/iptables.rules`，重啟 `iptables.service` |
| `nftables.service` 單獨作用中 | 套用 `wifimic-server.nft` 規則集 |
| `ufw.service` 作用中（且無 iptables/nftables 服務） | 使用 `ufw` 插入 peer allow 規則於第 1 優先序，附加 port-scoped deny 規則，執行 `ufw reload` |
| `ufw.service` 與 iptables/nftables 服務**同時**作用中 | **中止**，拒絕建立第二條封包過濾路徑 |
| `iptables.service` 與 `nftables.service` **同時**作用中 | **中止**，拒絕猜測或變更防火牆狀態 |
| 無支援防火牆服務作用中 | 回退安裝 `nftables`，套用 nft 規則並啟用 `nftables.service` |

**持久化要求**：三種後端的規則都必須在重開機後存活——UFW 規則由 `ufw` 自行持久化（寫入 `/etc/ufw/user.rules`）；iptables 情境由腳本以 `iptables-save` 寫入 `/etc/iptables/iptables.rules` 並重啟 `iptables.service` 載入；nftables 情境由 `nftables.service` 開機時載入規則集。請勿手動另啟第二套防火牆服務，避免雙重過濾路徑。

### 6.3 驗證規則生效

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

---

## 7. 健康檢查

### 7.1 服務與捕獲管線

```bash
# 服務狀態
systemctl --user is-active wifimic-server
# 應輸出 active

# 即時 journal 觀察捕獲啟動
journalctl --user -u wifimic-server -f
# 應見到 parec 啟動、無 EndpointNotFound 錯誤
```

### 7.2 網路連通性（從 Windows 對等端）

> 注意：`Test-NetConnection` 僅支援 TCP，無法測試 UDP。請改用下列 .NET `UdpClient` 方式送出測試資料包：

```powershell
# 從 Windows 對等端送出一個測試資料包（UDP 無連線，伺服器不會回覆）
$udp = New-Object System.Net.Sockets.UdpClient
$udp.Connect('192.168.0.210', 6902)
[void]$udp.Send([byte[]]@(0x01, 0x01), 2)
$udp.Close()
```

UDP 測試無法從 Windows 端單獨確認送達；請接著以第 7.3 節的防火牆計數器在 Linux 主機上確認封包確實抵達並被允許規則接受。

### 7.3 防火牆計數器驗證

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

## 8. 疑難排解

| 現象 | 可能原因 | 排查步驟 |
|------|----------|----------|
| `systemctl --user status wifimic-server` 顯示 `203/EXEC` | 二進位檔路徑錯誤或未安裝 | 確認 `~/.local/bin/wifimic_server` 存在且可執行；重跑第 3 節安裝步驟 |
| 服務啟動但 journal 出現 `EndpointNotFound` | 固定 PipeWire 來源不存在 | 執行第 4 節驗證指令；檢查 `pipewire`、`wireplumber` 服務狀態 |
| `loginctl enable-linger` 後服務仍在登出時停止 | linger 未生效 | `loginctl show-user daniel --property=Linger` 確認為 `yes`；重啟使用者管理器 `systemctl --user daemon-reload` |
| 防火牆腳本中止並顯示「both active」 | 多重防火牆服務同時啟用 | `systemctl list-units 'iptables.service' 'nftables.service' 'ufw.service' --state=active` 確認僅有一個作用中；停用多餘服務 |
| UFW 規則顯示 `ALLOW IN Anywhere` | 既有規則過寬 | 腳本會偵測並中止；手動 `ufw delete` 該規則後重跑腳本 |
| Windows 端無法連線 | 對等端 IP 不符或防火牆阻擋 | 確認 Windows 端 IP 確為 `192.168.0.200`；檢查防火牆計數器是否有 deny 遞增 |

---

## 9. 更新與復原

### 9.1 更新伺服器二進位檔

```bash
cd ~/src/wifimic
git fetch origin && git pull
cargo build --release --bin wifimic_server
cp target/release/wifimic_server ~/.local/bin/
systemctl --user restart wifimic-server
```

### 9.2 復原防火牆規則

**UFW**：

```bash
# 移除 wifimic 規則（依編號刪除，編號以 ufw status numbered 確認）
ufw delete <allow-rule-number>
ufw delete <deny-rule-number>
ufw reload
```

**nftables**：

```bash
sudo nft delete table inet wifimic_server
# 若為回退安裝的 nftables.service，可選擇停用
# sudo systemctl disable --now nftables.service
```

**iptables**：

```bash
sudo iptables -D INPUT -p udp -s 192.168.0.200 --dport 6902 -j ACCEPT
sudo iptables -D INPUT -p udp --dport 6902 -j DROP
sudo iptables-save | sudo tee /etc/iptables/iptables.rules >/dev/null
sudo systemctl restart iptables.service
```

### 9.3 完整移除部署

```bash
# 停用並移除 systemd 使用者單元
systemctl --user disable --now wifimic-server
rm ~/.config/systemd/user/wifimic-server.service
systemctl --user daemon-reload

# 移除二進位檔
rm ~/.local/bin/wifimic_server

# 移除防火牆規則（依上節對應後端執行）

# 取消使用者殘留（若無其他服務需要）
loginctl disable-linger daniel
```

---

## 10. 關鍵數值對照表

| 項目 | 數值 | 來源 |
|------|------|------|
| 服務名稱 | `wifimic-server` | `deploy/systemd/wifimic-server.service` |
| 二進位檔路徑 | `%h/.local/bin/wifimic_server` | systemd 單元 `ExecStart` |
| Linux 主機 IP | `192.168.0.210` | 部署環境固定 |
| Windows 對等端 IP | `192.168.0.200` | `apps/wifimic_server/src/network.rs:16`、`deploy/linux/wifimic-server-firewall.sh:4` |
| UDP 埠號 | `6902` | `apps/wifimic_server/src/network.rs:5`、`deploy/linux/wifimic-server-firewall.sh:5` |
| 固定捕獲源 | `alsa_input.pci-0000_00_1b.0.analog-stereo` | `apps/wifimic_server/src/capture_types.rs:6` |
| `parec` 參數 | `--raw --format=s16le --rate=48000 --channels=1 --latency-msec=5 --process-time-msec=5 --device=alsa_input.pci-0000_00_1b.0.analog-stereo` | `apps/wifimic_server/src/capture_types.rs:9-17` |

---

## 11. 限制與已知問題

1. **無第三方 LAN 來源驗證**：部署環境僅有 `192.168.0.200` 一個對等端，port-scoped deny 計數器未經實測遞增；規則邏輯經程式碼審查確認正確。
2. **非乾淨主機狀態**：`arch-daniel` 已有既有套件與服務狀態，本指南未涵蓋全新安裝情境；若在乾淨主機部署，請先安裝 `pipewire`、`wireplumber`、`rustup` 等基礎套件。
3. **Rust LSP 逾時**：工作區 Rust LSP 會逾時，不影響 `cargo build`、`cargo test`、`cargo clippy` 正常運作。
4. **認證路徑禁止**：本指南不包含任何認證相關路徑；請勿將憑證寫入持久化檔案。

---

## 12. 參考檔案

- `deploy/systemd/wifimic-server.service` — systemd 使用者單元定義
- `deploy/linux/wifimic-server-firewall.sh` — 防火牆部署主腳本（含後端選擇邏輯）
- `deploy/linux/wifimic-server-iptables.sh` — iptables 規則定義
- `deploy/linux/wifimic-server.nft` — nftables 規則集
- `apps/wifimic_server/src/capture_types.rs` — 固定捕獲源與 `parec` 參數
- `apps/wifimic_server/src/network.rs` — UDP 連接埠、對等端 IP、來源 IP 驗證邏輯

---

*文件版本：以 git 歷史為準（於倉庫根目錄執行 `git log -1 -- docs/deployment-linux.md` 可查得本檔案最後修訂提交）*
*部署驗證日期：2026-08-22*