# ResinCraft Launcher：功能

由 **Puirkle** 打造的现代、易用的 Minecraft 启动器，支持 Windows、macOS 和 Linux。没有遥测，也不需要 ResinCraft 账户。

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

标有 **(1.2.7)** 的是 1.2.7 版本的新功能。

## 游玩

- **实例：** 每个版本或整合包单独安装，各自拥有自己的模组、世界、Java、内存和 JVM 参数。
- **任意 Minecraft 版本：** 所有正式版和快照，一直到早期的经典版 Alpha。
- **模组加载器：** Fabric、Quilt、Forge（最早到 Minecraft 1.2.5）、NeoForge 和 LiteLoader，可以选择加载器版本。也支持 OptiFine 和 jar 模组。
- **更改版本和加载器 (1.2.7)：** 像 Prism 一样，在实例的**设置**标签页里切换 Minecraft 版本或模组加载器。点击实例页面上的版本标签也能打开。
- **启动：** 一键开玩。可以换用其他账户游玩，也可以用两个账户同时运行同一个实例。
- **快速加入：** 列出你最近玩过的服务器和单人世界，一键直接进入。
- **世界：** 直接进入世界游玩、重命名、备份，或从 zip 导入世界。
- **崩溃修复 (1.2.7)：** Minecraft 崩溃时，ResinCraft 会读取崩溃报告和日志，用简单的话告诉你出了什么问题，并提供一键修复。可以修复：安装缺少的模组、关闭导致崩溃的模组、选择合适的 Java、给游戏更多内存。
- **Java：** 自动找到已安装的 Java，按 Minecraft 版本选择合适的那个；都不合适时会下载 Eclipse Temurin。
- **内存显示：** 显示启动器、每个游戏和每个服务器用了多少内存（可选在主页显示）。
- **Discord Rich Presence：** 显示整合包名称、版本、游玩时长，以及在玩单人还是服务器。给好友的按钮：**View modpack**（查看整合包）和 **Get ResinCraft**（获取 ResinCraft）。

## 实例

- **整理：** 分组、自定义顺序（拖动排列）、图标、横幅和颜色。
- **标签页：** 内容、世界、服务器、截图、笔记、文件（内置文件管理器和编辑器）、日志和设置。
- **从其他实例复制：** 世界、模组及其设置（config 文件夹）、光影包、资源包和游戏设置。
- **创建副本、导出**（Modrinth `.mrpack` 或 CurseForge `.zip`），以及**做成你自己的整合包**。
- **命令和环境变量：** 设置启动前和游戏关闭后运行的命令，以及环境变量。
- **即时开关 (1.2.7)：** 开启或关闭模组立即生效，重命名在后台完成。

## 内容与发现

- **来源网站：** 来自 Modrinth、CurseForge、FTB、ATLauncher、Technic 和 GitHub Releases 的模组、整合包、资源包和光影，按实例的版本和加载器筛选。
- **整合包页面：** 包含简介、图库、版本、更新日志，以及**整合包里包含的模组**。
- **智能安装：** 任选版本，一次安装多个，自动添加必需的前置模组，还能发现从两个网站装了同一个模组的情况。
- **更新：** 检查并应用更新（单个或全部），支持整合包更新和版本切换。
- **手动下载：** 作者禁止启动器下载的 CurseForge 文件，会从你的下载文件夹或其他**监视文件夹**中自动获取（设置 → 下载）。
- **列表：** 把模组、资源包或光影导出为列表，再把列表安装到另一个实例。

## 开服

- **服务器类型：** Vanilla、Paper、Purpur、Folia、Spigot、Leaf、Fabric、Quilt、Forge、NeoForge、Arclight（模组和插件同时使用）以及 Bedrock Dedicated Server。
- **从整合包或实例创建：** 用 Modrinth、CurseForge（有官方服务端包时使用它）或 FTB 的整合包创建服务器，也可以从你的某个实例创建。
- **自动关闭仅客户端模组：** 每次服务器启动时，自动找出只能在游戏客户端运行的模组并关闭。其他模组依赖的模组会保持开启。如果仍有模组让服务器崩溃，会把它关闭并自动重启服务器。
- **管理：** 控制台、属性编辑器、支持更新的插件和模组管理、白名单、管理员和封禁、服务器图标、备份。
- **重启和更多菜单 (1.2.7)：** Stop（停止）旁边有“重新启动”；菜单里有复制地址、打开文件夹、备份、设置和删除。
- **当前在线：** 每隔几秒实时查看服务器上的玩家 **(1.2.7)**。
- **Java 参数 (1.2.7)：** 使用 ResinCraft 调好的参数，或整合包自带的参数（`user_jvm_args.txt`、启动脚本）。
- **与好友共享：**
  - ResinCraft Relay（`<name>.rscraft.resincore.tech`）
  - playit.gg
  - bore
  - 你的路由器（UPnP）
  - 你自己的域名，支持 Cloudflare DNS 自动配置
- **基岩版玩家也能加入** Java 版服务器（Geyser + Floodgate）。
- **Whitelist requests（白名单申请） (1.2.7)：** 玩家申请加入，你在 **Players** 标签页中同意或拒绝。
  - **Web page（网页）：** `https://rscraft.resincore.tech/join/<name>`（使用 ResinCraft Relay 时）。
  - **你自己的 Discord 机器人：** 玩家输入 `/whitelist`。
  - **你的答复：** 同意后会把玩家加入白名单，机器人会私信通知对方。
  - **保护：** 被封禁的玩家会被拒绝；**Reject and block**（拒绝并屏蔽）可以让对方无法再次申请。

## 基岩版（Windows）

- 安装、启动和卸载 Minecraft for Windows 与 Minecraft Preview（卸载前会先备份世界）。

## 账户

- **Microsoft 登录**，令牌加密保存。还有用于单人和局域网的**离线账户**。
- **皮肤、披风和改名。**

## 从其他启动器迁移

- 从 Prism Launcher、MultiMC、Modrinth App、CurseForge 应用、ATLauncher、GDLauncher 和官方 Minecraft Launcher 导入实例，连同模组、世界、资源包和设置一起。服务器也可以导入。

## 应用

- **Windows、macOS**（Apple Silicon 和 Intel）**和 Linux**（x64 和 ARM64：AppImage、`.deb`、`.tar.gz`）。
- **更新：** 自动检查。
  - **(1.2.7)** 标题栏中的**更新**按钮。
  - **(1.2.7)** 有游戏或服务器在运行时，可以**关闭并更新**。
  - **(1.2.7)** 在后台**静默更新**。
- **在系统托盘中保持运行 (1.2.7)：** 关闭窗口后，服务器、Discord 机器人和下载会继续运行。
- **16 种语言：** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **存储管理**（把数据移到其他硬盘）、设置的**备份与恢复**、**浅色和深色**主题。
