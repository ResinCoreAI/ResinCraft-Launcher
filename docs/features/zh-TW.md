# ResinCraft Launcher：功能

由 **Puirkle** 打造的現代、好上手的 Minecraft 啟動器，支援 Windows、macOS 和 Linux。沒有遙測，也不需要 ResinCraft 帳戶。

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

標有 **(1.2.7)** 的是 1.2.7 版的新功能。

## 遊玩

- **實例：** 每個版本或模組包各自獨立安裝，各有自己的模組、世界、Java、記憶體和 JVM 參數。
- **任何 Minecraft 版本：** 所有正式版和快照，一路到早期的經典 Alpha 版。
- **模組載入器：** Fabric、Quilt、Forge（最早到 Minecraft 1.2.5）、NeoForge 和 LiteLoader，可選擇載入器版本。也支援 OptiFine 和 jar 模組。
- **變更版本和載入器 (1.2.7)：** 和 Prism 一樣，在實例的**設定**分頁中切換 Minecraft 版本或模組載入器。點擊實例頁面上的版本標籤也能開啟。
- **遊玩：** 一鍵開玩。可以改用其他帳戶遊玩，也能用兩個帳戶同時執行同一個實例。
- **快速加入：** 列出你最近玩過的伺服器和單人世界，一鍵直接進入。
- **世界：** 直接進入世界遊玩、重新命名、備份，或從 zip 匯入世界。
- **當機修正 (1.2.7)：** Minecraft 當機時，ResinCraft 會讀取當機報告和記錄，用簡單的話告訴你出了什麼問題，並提供一鍵修正。可以修正：安裝缺少的模組、關閉造成當機的模組、選擇合適的 Java、給遊戲更多記憶體。
- **Java：** 自動找到已安裝的 Java，依 Minecraft 版本挑選合適的；都不合適時會下載 Eclipse Temurin。
- **記憶體顯示：** 顯示啟動器、每個遊戲和每個伺服器用了多少記憶體（可選擇在首頁顯示）。
- **Discord Rich Presence：** 顯示模組包名稱、版本、遊玩時間，以及在玩單人還是伺服器。給好友的按鈕：**View modpack**（查看模組包）和 **Get ResinCraft**（取得 ResinCraft）。

## 實例

- **整理：** 群組、自訂順序（拖曳排列）、圖示、橫幅和顏色。
- **分頁：** 內容、世界、伺服器、螢幕截圖、筆記、檔案（內建檔案管理器和編輯器）、日誌和設定。
- **從其他實例複製：** 世界、模組及其設定（config 資料夾）、光影包、資源包和遊戲設定。
- **建立副本、匯出**（Modrinth `.mrpack` 或 CurseForge `.zip`），以及**做成你自己的模組包**。
- **指令與環境變數：** 設定啟動前和遊戲關閉後要執行的指令，以及環境變數。
- **即時開關 (1.2.7)：** 開啟或關閉模組立即生效，重新命名在背景完成。

## 內容與探索

- **來源網站：** 來自 Modrinth、CurseForge、FTB、ATLauncher、Technic 和 GitHub Releases 的模組、模組包、資源包和光影，依實例的版本和載入器篩選。
- **模組包頁面：** 包含簡介、圖庫、版本、更新記錄，以及**模組包裡包含的模組**。
- **聰明安裝：** 任選版本、一次安裝多個、自動加入必要的前置模組，還能發現從兩個網站裝了同一個模組的情況。
- **更新：** 檢查並套用更新（單個或全部），支援模組包更新和版本切換。
- **手動下載：** 作者禁止啟動器下載的 CurseForge 檔案，會從你的下載資料夾或其他**監視資料夾**中自動取得（設定 → 下載）。
- **清單：** 把模組、資源包或光影匯出成清單，再把清單安裝到另一個實例。

## 架設伺服器

- **伺服器類型：** Vanilla、Paper、Purpur、Folia、Spigot、Leaf、Fabric、Quilt、Forge、NeoForge、Arclight（模組和插件同時使用）以及 Bedrock Dedicated Server。
- **從模組包或實例建立：** 用 Modrinth、CurseForge（有官方伺服器包時使用它）或 FTB 的模組包建立伺服器，也可以從你的某個實例建立。
- **自動關閉僅限用戶端的模組：** 每次伺服器啟動時，自動找出只能在遊戲用戶端執行的模組並關閉。其他模組需要的模組會保持開啟。如果仍有模組讓伺服器當機，會把它關閉並自動重新啟動伺服器。
- **管理：** 主控台、屬性編輯器、支援更新的插件和模組管理、白名單、管理員和封鎖、伺服器圖示、備份。
- **重新啟動和更多選單 (1.2.7)：** Stop（停止）旁邊有「重新啟動」；選單中有複製位址、開啟資料夾、備份、設定和刪除。
- **目前線上：** 每隔幾秒即時查看伺服器上的玩家 **(1.2.7)**。
- **Java 參數 (1.2.7)：** 使用 ResinCraft 調校好的參數，或模組包自帶的參數（`user_jvm_args.txt`、啟動腳本）。
- **與好友分享：**
  - ResinCraft Relay（`<name>.rscraft.resincore.tech`）
  - playit.gg
  - bore
  - 你的路由器（UPnP）
  - 你自己的網域，支援 Cloudflare DNS 自動設定
- **基岩版玩家也能加入** Java 版伺服器（Geyser + Floodgate）。
- **Whitelist requests（白名單申請） (1.2.7)：** 玩家申請加入，你在 **Players** 分頁中接受或拒絕。
  - **Web page（網頁）：** `https://rscraft.resincore.tech/join/<name>`（使用 ResinCraft Relay 時）。
  - **你自己的 Discord 機器人：** 玩家輸入 `/whitelist`。
  - **你的回覆：** 接受後會把玩家加入白名單，機器人會私訊通知對方。
  - **保護：** 被封鎖的玩家會被拒絕；**Reject and block**（拒絕並封鎖）可以讓對方無法再次申請。

## 基岩版（Windows）

- 安裝、啟動和解除安裝 Minecraft for Windows 與 Minecraft Preview（解除安裝前會先備份世界）。

## 帳戶

- **Microsoft 登入**，權杖加密保存。還有用於單人和區域網路的**離線帳戶**。
- **外觀、披風和改名。**

## 從其他啟動器搬移

- 從 Prism Launcher、MultiMC、Modrinth App、CurseForge 應用程式、ATLauncher、GDLauncher 和官方 Minecraft Launcher 匯入實例，連同模組、世界、資源包和設定一起。伺服器也可以匯入。

## 應用程式

- **Windows、macOS**（Apple Silicon 和 Intel）**和 Linux**（x64 和 ARM64：AppImage、`.deb`、`.tar.gz`）。
- **更新：** 自動檢查。
  - **(1.2.7)** 標題列中的**更新**按鈕。
  - **(1.2.7)** 有遊戲或伺服器在執行時，可以**關閉並更新**。
  - **(1.2.7)** 在背景**靜默更新**。
- **在系統匣中保持執行 (1.2.7)：** 關閉視窗後，伺服器、Discord 機器人和下載會繼續執行。
- **16 種語言：** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **儲存空間管理**（把資料移到其他磁碟）、設定的**備份與還原**、**淺色和深色**主題。
