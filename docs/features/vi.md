# ResinCraft Launcher: tính năng

Launcher Minecraft hiện đại, dễ dùng, do **Puirkle** làm, chạy trên Windows, macOS và Linux. Không thu thập dữ liệu sử dụng (telemetry) và không cần tài khoản ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Các mục có đánh dấu **(1.2.7)** là mới trong phiên bản 1.2.7.

## Chơi game

- **Bản cài đặt:** mỗi phiên bản hoặc modpack được cài riêng, có mod, thế giới, Java, bộ nhớ và tham số JVM của riêng nó.
- **Mọi phiên bản Minecraft:** mọi bản phát hành và snapshot, lùi về tận các bản alpha cổ điển.
- **Trình tải mod:** Fabric, Quilt, Forge (về tận Minecraft 1.2.5), NeoForge và LiteLoader, có chọn phiên bản trình tải. Hỗ trợ cả OptiFine và jar mod.
- **Đổi phiên bản và trình tải (1.2.7):** đổi phiên bản Minecraft hoặc trình tải mod của bản cài ngay trong thẻ Cài đặt, giống như Prism. Bấm vào nhãn phiên bản trên trang bản cài cũng mở ra chỗ này.
- **Chơi:** chỉ một cú nhấp. Dùng **Lần này chơi bằng** để chơi bằng tài khoản khác, hoặc chạy cùng một bản cài hai lần với hai tài khoản.
- **Vào nhanh:** các máy chủ và thế giới chơi đơn gần đây, một cú nhấp là vào thẳng game.
- **Thế giới:** chơi thẳng một thế giới, đổi tên, sao lưu, nhập thế giới từ file zip.
- **Sửa lỗi crash (1.2.7):** khi Minecraft bị crash, ResinCraft đọc báo cáo crash và nhật ký, giải thích lỗi bằng lời dễ hiểu và cho bạn sửa chỉ với một cú nhấp. Có thể sửa: cài mod còn thiếu, tắt mod gây crash, chọn đúng Java, cấp thêm bộ nhớ cho game.
- **Java:** tìm các bản Java đã cài, chọn đúng bản cho từng phiên bản Minecraft và tải Eclipse Temurin khi không có bản nào phù hợp.
- **Hiển thị RAM:** launcher, từng game và từng máy chủ đang dùng bao nhiêu bộ nhớ (có thể hiện ở Trang chủ).
- **Discord Rich Presence:** tên gói, phiên bản, thời gian chơi, đang chơi đơn hay trên máy chủ. Có nút cho bạn bè: **View modpack** và **Get ResinCraft** (nút trên Discord, giữ tiếng Anh).

## Bản cài đặt

- **Sắp xếp:** nhóm, thứ tự của riêng bạn (kéo để sắp xếp), biểu tượng, banner và màu sắc.
- **Các thẻ:** Nội dung, Thế giới, Máy chủ, Ảnh chụp, Ghi chú, Tệp (trình quản lý và chỉnh sửa tệp có sẵn), Nhật ký và Cài đặt.
- **Sao chép từ bản cài khác:** thế giới, mod và cài đặt của chúng (thư mục config), gói shader, gói tài nguyên và cài đặt trò chơi.
- **Nhân bản, xuất** (Modrinth `.mrpack` hoặc CurseForge `.zip`) và **biến nó thành modpack của riêng bạn**.
- **Lệnh và biến môi trường:** lệnh chạy trước khi khởi động và sau khi đóng game, cùng các biến môi trường.
- **Bật tắt tức thì (1.2.7):** bật hay tắt mod đều có tác dụng ngay; việc đổi tên tệp diễn ra ở nền.

## Nội dung và Khám phá

- **Nguồn:** mod, modpack, gói tài nguyên và shader từ Modrinth, CurseForge, FTB, ATLauncher, Technic và bản phát hành trên GitHub, được lọc theo phiên bản và trình tải của bản cài.
- **Trang modpack** có mô tả, thư viện ảnh, các phiên bản, nhật ký thay đổi và **danh sách mod có trong gói**.
- **Cài đặt thông minh:** chọn phiên bản nào cũng được, cài nhiều thứ cùng lúc, mod phụ thuộc bắt buộc được thêm tự động, và phát hiện khi cùng một mod đến từ hai trang.
- **Cập nhật:** kiểm tra và cập nhật (từng cái hoặc tất cả); cập nhật modpack và đổi phiên bản.
- **Tải thủ công:** tệp CurseForge mà tác giả không cho launcher tải sẽ được lấy từ thư mục Tải xuống hoặc các **thư mục theo dõi** khác (Cài đặt → Tải xuống).
- **Danh sách:** xuất mod, gói tài nguyên hoặc shader thành danh sách, rồi cài danh sách đó vào bản cài khác.

## Tự mở máy chủ

- **Loại máy chủ:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mod và plugin cùng lúc) và Bedrock Dedicated Server.
- **Từ modpack hoặc bản cài:** tạo máy chủ từ Modrinth, CurseForge (dùng gói máy chủ chính thức nếu có) hoặc FTB, hoặc từ một bản cài của bạn.
- **Tự tắt mod chỉ dành cho client:** mod chỉ chạy trên client game được tìm ra và tắt tự động mỗi lần máy chủ khởi động. Mod nào mà mod khác cần thì vẫn được giữ bật. Nếu vẫn có mod làm máy chủ crash, mod đó sẽ bị tắt và máy chủ tự khởi động lại.
- **Quản lý:** console, trình chỉnh sửa properties, quản lý plugin và mod kèm cập nhật, whitelist, op và danh sách cấm, biểu tượng máy chủ, sao lưu.
- **Khởi động lại và menu thêm (1.2.7):** nút Khởi động lại nằm cạnh nút Dừng; menu có sao chép địa chỉ, mở thư mục, sao lưu, cài đặt và xóa.
- **Online now** (đang online): người chơi trên máy chủ của bạn, được kiểm tra trực tiếp mỗi vài giây **(1.2.7)**.
- **Java flags (1.2.7)** (cờ Java): bộ cờ đã tinh chỉnh của ResinCraft, hoặc cờ riêng của modpack (`user_jvm_args.txt`, script khởi động).
- **Chơi cùng bạn bè:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - router của bạn (UPnP)
  - tên miền riêng của bạn, tự động cấu hình DNS trên Cloudflare
- **Bedrock players can join** (người chơi Bedrock vào được): người chơi Bedrock vào được máy chủ Java (Geyser + Floodgate).
- **Whitelist requests (1.2.7)** (yêu cầu vào whitelist): người chơi xin vào, còn bạn chấp nhận hoặc từ chối trong thẻ Players.
  - **Trang web:** `https://rscraft.resincore.tech/join/<name>` (khi dùng ResinCraft Relay).
  - **Bot Discord của riêng bạn:** người chơi gõ `/whitelist`.
  - **Câu trả lời của bạn:** Accept đưa họ vào whitelist; bot báo cho họ qua tin nhắn riêng.
  - **Bảo vệ:** người chơi bị cấm sẽ bị từ chối; **Reject and block** chặn người đó gửi yêu cầu thêm lần nữa.

## Bedrock Edition (Windows)

- Cài, chạy và gỡ Minecraft for Windows và Minecraft Preview (thế giới được sao lưu trước).

## Tài khoản

- **Đăng nhập bằng Microsoft**, token được lưu ở dạng mã hóa. **Hồ sơ ngoại tuyến** để chơi đơn và LAN.
- **Skin, áo choàng và đổi tên.**

## Chuyển từ launcher khác

- Nhập bản cài từ Prism Launcher, MultiMC, Modrinth App, ứng dụng CurseForge, ATLauncher, GDLauncher và Minecraft Launcher chính thức, kèm mod, thế giới, gói và cài đặt. Cũng nhập được cả máy chủ.

## Ứng dụng

- **Windows, macOS** (Apple Silicon và Intel) **và Linux** (x64 và ARM64: AppImage, `.deb`, `.tar.gz`).
- **Cập nhật:** tự động kiểm tra.
  - **(1.2.7)** Nút **Cập nhật** trên thanh tiêu đề.
  - **(1.2.7)** **Đóng và cập nhật** khi đang có game hoặc máy chủ chạy.
  - **(1.2.7)** **Cập nhật âm thầm** ở nền.
- **Tiếp tục chạy ở khay hệ thống (1.2.7):** đóng cửa sổ thì máy chủ, bot Discord và tải xuống vẫn tiếp tục.
- **16 ngôn ngữ:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Quản lý lưu trữ** (chuyển dữ liệu sang ổ khác), **sao lưu và khôi phục** cài đặt, giao diện **sáng và tối**.
