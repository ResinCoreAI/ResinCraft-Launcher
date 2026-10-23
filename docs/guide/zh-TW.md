# ResinCraft Launcher：使用指南

這裡說明在 ResinCraft 中最常做的事。**粗體**名稱就是你在畫面上看到的名稱。

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## 目錄

1. [安裝與登入](#1-安裝與登入)
2. [建立第一個實例](#2-建立第一個實例)
3. [加入模組、資源包和光影](#3-加入模組資源包和光影)
4. [安裝模組包](#4-安裝模組包)
5. [變更 Minecraft 版本或模組載入器](#5-變更-minecraft-版本或模組載入器)
6. [Minecraft 當機時](#6-minecraft-當機時)
7. [從其他實例複製內容](#7-從其他實例複製內容)
8. [架設伺服器](#8-架設伺服器)
9. [讓好友加入你的伺服器](#9-讓好友加入你的伺服器)
10. [白名單申請（網頁和 Discord 機器人）](#10-白名單申請網頁和-discord-機器人)
11. [基岩版](#11-基岩版)
12. [從其他啟動器搬移](#12-從其他啟動器搬移)
13. [更新與系統匣](#13-更新與系統匣)
14. [疑難排解](#14-疑難排解)

## 1. 安裝與登入

1. 從 [Releases 頁面](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest)下載適合你系統的 ResinCraft：
   - **Windows：** `…_amd64_windows.exe`
   - **macOS：** `…_universal_macos.dmg`
   - **Linux：** `.AppImage`、`.deb` 或 `.tar.gz`
2. 安裝並開啟。Windows 可能會提示發行者未經驗證；在 macOS 上，第一次開啟時請在應用程式上按右鍵並選擇**開啟**。
3. 開啟**帳戶** → **使用 Microsoft 登入**，在 Microsoft 自己的頁面上登入。你的密碼不會經過 ResinCraft。
   - 沒有 Microsoft 帳戶？可以新增一個**離線帳戶**，用於單人遊戲和區域網路。

## 2. 建立第一個實例

*實例*是一份獨立的 Minecraft 安裝，有自己的模組、世界和設定。

1. 開啟**遊戲庫** → **新增實例**。
2. 選擇 **Minecraft 版本**和**模組載入器**：
   - **Vanilla：** 不裝模組。
   - **Fabric** 或 **Quilt：** 輕量模組、效能模組。
   - **Forge** 或 **NeoForge：** 大型內容模組。
3. 取個名字，然後按**建立實例**。
4. 按**遊玩**。第一次啟動會下載遊戲和合適的 Java，之後啟動就很快了。

小提示：左側的**快速加入**會列出你最近玩過的伺服器和世界，一鍵直接進入。

## 3. 加入模組、資源包和光影

1. 開啟你的實例 → **內容**分頁 → **新增模組**。也可以從左側欄開啟**探索**。
2. 搜尋，或瀏覽 Modrinth、CurseForge、FTB 等網站。只會顯示適合你實例的版本。
3. 按**安裝**。必要的模組（前置）會自動幫你加入。
4. 回到**內容**：
   - 用模組旁邊的開關來開啟或關閉它；
   - 按**檢查更新**來更新模組（單個或全部）。

有些 CurseForge 作者不允許啟動器下載他們的檔案。這時 ResinCraft 會把這些檔案列在**全部下載**底下：在瀏覽器中下載它們，ResinCraft 會自動在你的下載資料夾裡找到。瀏覽器存到別的地方？請在**設定 → 下載 → 監視資料夾**中加入那個資料夾。

## 4. 安裝模組包

1. 開啟**探索** → **模組包**（或**遊戲庫** → **新增實例** → **下載模組包**）。
2. 開啟一個模組包，查看簡介、版本以及其中包含的**模組**。
3. 按**安裝模組包**。ResinCraft 會建立一個包含所有內容的新實例。
4. 模組包有更新時，ResinCraft 會通知你；你也可以在實例頁面選擇其他模組包版本。

## 5. 變更 Minecraft 版本或模組載入器

1. 在實例頁面，點擊頂端的版本標籤（例如 **1.21.4 · Fabric 0.16.10**），會開啟**設定 → 版本**。
2. 選擇 **Minecraft** 版本、**模組載入器**和載入器版本。
3. 按**變更版本**並確認。下次啟動時會下載所需的檔案。
4. 為舊版本製作的模組可能無法使用：前往**內容**，按**檢查更新**。

## 6. Minecraft 當機時

遊戲當機時，會跳出「**（實例名稱） 當機了**」視窗。ResinCraft 會讀取當機報告和記錄，告訴你出了什麼問題，例如（這些訊息會以英文顯示）：

- *Sodium Extra needs Sodium, which isn't installed*（Sodium Extra 需要 Sodium，但尚未安裝）→ **修正**會安裝 Sodium。
- *citresewn crashed the game*（citresewn 造成遊戲當機）→ **修正**會關閉該模組（之後可以在**內容**中重新開啟）。
- *This needs Java 21 or newer*（需要 Java 21 或更新版本）→ **修正**會讓 ResinCraft 選擇合適的 Java。
- *Minecraft ran out of memory*（Minecraft 記憶體不足）→ **修正**會給它更多記憶體。

逐一按**修正**，或按**全部修正並遊玩**。在你按下按鈕之前，什麼都不會改變。如果 ResinCraft 找不出原因，可以用**開啟記錄**查看遊戲最後輸出的內容。

## 7. 從其他實例複製內容

1. 開啟你要複製*到*的實例 → 按頂端的**從其他實例複製**按鈕。
2. 選擇要*從*哪個實例複製。
3. 勾選你想要的內容：
   - **世界**；
   - **模組**和**模組設定**（config 資料夾）；
   - **光影包**和**資源包**；
   - **遊戲設定**（視訊、控制、按鍵綁定）。
4. 按**複製**。名稱已存在的世界會換一個新名字。你已經有的模組和資源包會維持原樣。

## 8. 架設伺服器

伺服器相關的畫面目前部分仍為英文。

1. 開啟 **Servers**（伺服器）→ **Create server**（建立伺服器）。
2. 選擇建立方式：
   - **Server software**（伺服器軟體）：Paper、Fabric、Forge、NeoForge、Vanilla、Bedrock…
   - **Modpack**（模組包）：和你的好友玩同一個模組包。
   - **From an instance**（從實例）：使用你已經在用的模組。
   - **From a file**（從檔案）。
3. 按 **Create**（建立）。視窗會關閉，下載會在右下角繼續進行。
4. 出現提示時同意 Minecraft EULA，然後按 **Start**（啟動）。

在伺服器頁面上：

- **Console**（主控台）：伺服器的輸出；在底部輸入指令。
- **Players**（玩家）：目前線上的玩家，以及白名單、管理員和封鎖清單。
- **Mods** 或 **Plugins**（模組／插件）：新增、更新或關閉。
- **Properties**（屬性）：伺服器設定（難度、白名單開關等）。
- **Settings**（設定）：
  - 記憶體、Java 和 **Java flags**（Java 參數：ResinCraft 的或模組包自帶的）；
  - 好友如何連線；
  - 伺服器圖示。
- **重新啟動**和 **⋯** 選單（複製位址、開啟資料夾、立即備份、刪除）。

只在遊戲用戶端執行的模組（光影、小地圖等）會在伺服器上自動關閉。你可以在 **Mods** 分頁中看到它們，需要時再重新開啟。

## 9. 讓好友加入你的伺服器

在伺服器的 **Settings** → **How friends connect**（好友如何連線）中選擇：

- **Automatic**（自動，建議）：先嘗試 **ResinCraft Relay**，接著是 playit.gg，再來是 bore。
- **ResinCraft Relay：** 好友透過 `yourname.rscraft.resincore.tech` 加入。需要登入 Microsoft 帳戶。
- **playit.gg：** 免費且固定不變的位址。第一次啟動時會要求你連結一個 playit.gg 帳戶（訪客帳戶也可以）。
- **我已自行轉送連接埠：** 如果你自己設定好了路由器。

**Console** 分頁會顯示要給好友的位址。基岩版好友也能加入 Java 版伺服器：開啟 **Bedrock players can join**（允許基岩版玩家加入）。

## 10. 白名單申請（網頁和 Discord 機器人）

讓玩家申請加入，由你自己決定接受或拒絕。

1. 在 **Properties** 中，把 **white-list** 設為 **true**（否則任何人都能直接加入）。
2. 開啟伺服器的 **Players** 分頁，開啟 **Whitelist requests**（白名單申請）。
3. 選擇玩家申請的方式：
   - **Web page**（網頁）：
     - 伺服器透過 ResinCraft Relay 分享時可用。
     - 按 **Copy link**（複製連結），把連結傳給玩家：`https://rscraft.resincore.tech/join/<name>`。
     - 伺服器在線上時頁面才能使用。
   - **Your Discord bot**（你的 Discord 機器人）：
     1. 開啟 [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token**，然後複製權杖。
     2. 貼到 ResinCraft 中並按 **Save**（儲存）。權杖會安全地保存在你的電腦上，不會再顯示。
     3. 顯示 **Online as …** 後，按 **Add the bot to your Discord**（把機器人加入你的 Discord），然後選擇你的 Discord 伺服器。
     4. 玩家在你的 Discord 中輸入 `/whitelist TheirName`。
4. 申請會顯示在 **Players** 分頁中：
   - **Accept**（接受）：把玩家加入白名單（機器人會私訊通知對方）。
   - **Reject**（拒絕）：拒絕這筆申請。
   - **Reject and block**（拒絕並封鎖）：拒絕申請，並阻止此人再次申請。**Unblock all**（全部解除封鎖）可以撤銷 Discord 上的封鎖。

被封鎖玩家的申請會自動被拒絕。Discord 機器人只在 ResinCraft 開啟時運作；開啟系統匣（第 13 節），關閉視窗後它也能繼續運作。

## 11. 基岩版

在 Windows 上，左側的**基岩版**可以安裝、開啟和解除安裝 Minecraft for Windows 與 Minecraft Preview。解除安裝前會先備份你的世界。

## 12. 從其他啟動器搬移

**遊戲庫** → **從其他啟動器搬移**可以匯入以下啟動器的實例：

- Prism Launcher 和 MultiMC
- Modrinth App 和 CurseForge 應用程式
- ATLauncher 和 GDLauncher
- 官方 Minecraft Launcher

模組、世界和設定會一起搬過來。原本的檔案會留在原處。

## 13. 更新與系統匣

- **更新：**
  - 有新的 ResinCraft 版本可用時，標題列會出現**更新**按鈕。
  - 如果有遊戲或伺服器正在執行，你可以選擇**關閉並更新**（伺服器會先儲存世界）或**停止後更新**。
- **靜默更新**（設定 → 更新）：在沒有任何東西執行時，不詢問直接在背景安裝新版本。
- **在系統匣中保持執行**（設定 → 外觀）：
  - 關閉視窗後 ResinCraft 仍會執行：伺服器、Discord 機器人和下載都會繼續。
  - 點擊系統匣圖示重新開啟；按右鍵 → **Quit ResinCraft**（結束 ResinCraft）才會真正結束。

## 14. 疑難排解

- **遊戲無法啟動：** 查看「**（實例名稱） 當機了**」視窗（第 6 節），或實例的**日誌**分頁。
- **在「探索」中找不到某個模組：** 它可能沒有適用於你的 Minecraft 版本或載入器的版本。試試其他版本，或變更實例的版本（第 5 節）。
- **好友無法加入：** 檢查伺服器 **Console** 分頁上的位址，並確認伺服器顯示 **Running**（執行中）。使用 ResinCraft Relay 時，請確認已登入 Microsoft 帳戶。
- **伺服器啟動時當機：** ResinCraft 會自動關閉僅限用戶端的模組並重新啟動伺服器。在 **Console** 中可以找到說明關閉了哪個模組的那一行。
- **還是解決不了？** 附上**日誌**分頁中的記錄，到 [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) 回報問題。
