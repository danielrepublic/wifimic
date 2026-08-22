這個專案將建立在 "C:\\Users\\Daniel\\Documents\\opencode\\micdriver" 的大規模重構上。 以目前專案路徑作為工作區域製作一款應用程序。

架構概念: linux電腦擷取收到的麥克風音訊 -> 內網 -> windows 主機上的 vb-audio-vitual-cable input -> vb-audio-vitual-cable output -> 提供應用程式(discord, obs...)使用

製作規範:

1. linux 端服務可透過指令從 github 取用程式部署在機器上，服務名稱是 wifimic-server
2. windows 端可透過指令從 github 取用程式部署在機器上，服務名稱是 wifimic-client，安裝位置在 C:\\Program Files\\wifimic-client
3. linux 端在 sudo systemctl start wifimic 後會啟動服務，但只有當 windows 端啟動 wifimic-client 後才會要求 linux 端開始串流音訊。
4. 兩端串流服務皆使用 port 6902。
5. windows 端得服務能夠開機自動啟動，並在背景運行。
6. windows 托盤(隱藏的圖示)中會顯示服務圖示，可以透過選單選擇 restart 或 exit。當按下 restart 後，則重新啟動服務並再次從 linux 端開始串流音訊。 exit 則是關閉 windows 端服務並使 linux 端停止擷取音訊，但 wifimic-server 仍保持開啟，直到 windows 端啟動服務再次請求音訊串流。 簡單來說就是linux 端服務永遠開啟，但是是否擷取音訊並串流則取決於 windows 端是否有啟動 clint 服務。
7. 所有機器皆是私人所有且位於相同內網下，不需考慮加密問題以降低串流延遲。

現有環境:

1. windows 端機器: 本機，單一使用者帳號的個人電腦，內網IP固定在192.168.0.200
2. linux 端機器: arch-daniel，單一使用者帳號的個人無GUI linux 電腦，內網IP固定在192.168.0.210，sudo 密碼位於/home/daniel/.psw，ssh連線可透過 ssh arch-daniel直連，設定位於 "C:\\Users\\Daniel.ssh\\config"。

製作應用程序時不需考慮通用性，優先能夠部署於兩台機器上正常運行即可。



所有本專案 opencode session 主要語言使用 繁體中文。 

