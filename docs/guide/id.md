# ResinCraft Launcher: panduan pengguna

Cara melakukan hal-hal yang paling umum di ResinCraft. Nama yang **ditebalkan** adalah yang akan Anda lihat di layar. Setiap bagian menyebutkan versi saat bagian itu hadir.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Daftar isi

1. [Pasang dan masuk](#1-pasang-dan-masuk)
2. [Buat instalasi pertama Anda](#2-buat-instalasi-pertama-anda)
3. [Tambahkan mod, paket sumber daya, dan shader](#3-tambahkan-mod-paket-sumber-daya-dan-shader)
4. [Pasang modpack](#4-pasang-modpack)
5. [Ganti versi Minecraft atau pemuat mod](#5-ganti-versi-minecraft-atau-pemuat-mod)
6. [Saat Minecraft crash](#6-saat-minecraft-crash)
7. [Salin dari instalasi lain](#7-salin-dari-instalasi-lain)
8. [Jalankan server sendiri](#8-jalankan-server-sendiri)
9. [Ajak teman masuk ke server Anda](#9-ajak-teman-masuk-ke-server-anda)
10. [Permintaan whitelist (halaman web dan bot Discord)](#10-permintaan-whitelist-halaman-web-dan-bot-discord)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Pindah dari launcher lain](#12-pindah-dari-launcher-lain)
13. [Pembaruan dan system tray](#13-pembaruan-dan-system-tray)
14. [Tema dan CSS kustom](#14-tema-dan-css-kustom)
15. [Kendali jarak jauh (konsol web)](#15-kendali-jarak-jauh-konsol-web)
16. [Instal ulang dan perbaiki](#16-instal-ulang-dan-perbaiki)
17. [Cari dan gabung ke server](#17-cari-dan-gabung-ke-server)
18. [Daftarkan server Anda dan bagikan mod-nya](#18-daftarkan-server-anda-dan-bagikan-mod-nya)
19. [PC Anda yang lain dan teman yang membantu](#19-pc-anda-yang-lain-dan-teman-yang-membantu)
20. [Cadangan dunia](#20-cadangan-dunia)
21. [Penyimpanan dan pindah ke PC lain](#21-penyimpanan-dan-pindah-ke-pc-lain)
22. [Pemecahan masalah](#22-pemecahan-masalah)

## 1. Pasang dan masuk

*Sejak versi 0.1.0.*

1. Unduh ResinCraft untuk sistem Anda dari [halaman Releases](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** [`ResinCraft-Setup.exe`](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/download/setup/ResinCraft-Setup.exe) (selalu memasang versi terbaru), atau `…_amd64_windows.exe` / `…_amd64_windows.msi`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb`, atau `.tar.gz`
2. Pasang lalu buka. Windows mungkin bilang penerbitnya belum terverifikasi; di macOS, klik kanan aplikasinya dan pilih **Buka** saat pertama kali.
3. Buka **Akun** → **Masuk dengan Microsoft**, lalu masuk di halaman Microsoft sendiri. Kata sandi Anda tidak pernah melewati ResinCraft.
   - Tidak punya akun Microsoft? Tambahkan **Profil offline** untuk pemain tunggal dan LAN.

## 2. Buat instalasi pertama Anda

*Sejak versi 0.1.0.*

*Instalasi* (instance) adalah pemasangan Minecraft terpisah dengan mod, dunia, dan pengaturannya sendiri.

1. Buka **Pustaka** → **Instalasi baru**.
2. Pilih **Versi Minecraft** dan **Loader mod**:
   - **Vanilla:** tanpa mod.
   - **Fabric** atau **Quilt:** mod ringan, mod performa.
   - **Forge** atau **NeoForge:** mod konten besar.
3. Beri nama lalu tekan **Buat instalasi**.
4. Tekan **Main**. Saat pertama kali, game dan Java yang tepat akan diunduh; setelah itu, game cepat terbuka.

Tips: **Gabung cepat** di sebelah kiri menampilkan server dan dunia terakhir Anda; sekali klik langsung masuk.

## 3. Tambahkan mod, paket sumber daya, dan shader

*Sejak versi 0.1.0.*

1. Buka instalasi Anda → tab **Konten** → **Tambah mod**. Atau buka **Jelajahi** dari bilah kiri.
2. Cari, atau jelajahi Modrinth, CurseForge, FTB, dan situs lainnya. Hanya versi yang cocok dengan instalasi Anda yang ditampilkan.
3. Tekan **Pasang**. Mod yang dibutuhkan (dependensi) ditambahkan untuk Anda.
4. Kembali di **Konten**:
   - gunakan sakelar di samping mod untuk menyalakan atau mematikannya;
   - tekan **Periksa pembaruan** untuk memperbarui mod (satu atau semua). **Perbarui semua** mengunduh hingga empat sekaligus.

Beberapa pembuat mod di CurseForge tidak mengizinkan launcher mengunduh file mereka. ResinCraft lalu menampilkannya di bawah **Unduh semua**: unduh lewat browser Anda, dan ResinCraft akan menemukannya sendiri di folder Unduhan. Browser Anda menyimpan di tempat lain? Tambahkan folder itu di **Pengaturan → Unduhan → Folder yang dipantau**.

## 4. Pasang modpack

*Sejak versi 0.1.0.*

1. Buka **Jelajahi** → **Modpack** (atau **Pustaka** → **Instalasi baru** → **Unduh modpack**).
2. Buka sebuah paket untuk melihat deskripsi, versi, dan **Mod** yang ada di dalamnya.
3. Tekan **Pasang paket**. Instalasi baru langsung muncul di **Pustaka**, dengan progres pemasangan di kartunya; **Main** bisa dipakai setelah selesai.
4. Saat paket mendapat pembaruan, ResinCraft akan memberi tahu Anda; Anda juga bisa memilih versi paket lain di halaman instalasi.

## 5. Ganti versi Minecraft atau pemuat mod

*Sejak versi 1.2.7.*

1. Di halaman instalasi, klik label versi di bagian atas (misalnya **1.21.4 · Fabric 0.16.10**). Ini membuka **Pengaturan → Versi**.
2. Pilih versi **Minecraft**, **Pemuat mod**, dan versi loader-nya.
3. Tekan **Ubah versi** lalu konfirmasi. Saat game dijalankan berikutnya, yang dibutuhkan akan diunduh.
4. Mod yang dibuat untuk versi lama mungkin tidak berfungsi: buka **Konten** dan tekan **Periksa pembaruan**.

## 6. Saat Minecraft crash

*Sejak versi 1.2.7.*

Saat game crash, jendela **(nama instalasi) crash** terbuka. ResinCraft membaca laporan crash dan log, lalu memberi tahu apa yang salah, misalnya:

- *Sodium Extra butuh Sodium, yang belum terpasang* → **Perbaiki** memasang Sodium.
- *citresewn membuat game crash* → **Perbaiki** mematikan mod itu (nyalakan lagi nanti di **Konten**).
- *Ini butuh Java 21 atau yang lebih baru* → **Perbaiki** membiarkan ResinCraft memilih Java yang tepat.
- *Minecraft kehabisan memori* → **Perbaiki** memberinya lebih banyak memori.

Tekan **Perbaiki** di masing-masing, atau **Perbaiki semua dan main**. Tidak ada yang berubah sampai Anda menekan tombol. Jika ResinCraft tidak tahu penyebabnya, **Buka log** menampilkan pesan terakhir dari game.

## 7. Salin dari instalasi lain

*Sejak versi 1.2.5.*

1. Buka instalasi *tujuan* salinan → tekan tombol **Salin dari instance lain** di bagian atas.
2. Pilih instalasi *sumber* salinan.
3. Centang yang Anda inginkan:
   - **Dunia**;
   - **Mod** dan **Pengaturan mod** (folder config);
   - **Shader pack** dan **Resource pack**;
   - **Pengaturan game** (video, kontrol, tombol pintas).
4. Tekan **Salin**. Dunia yang namanya sudah dipakai akan mendapat nama baru. Mod dan paket yang sudah Anda punya dibiarkan apa adanya.

## 8. Jalankan server sendiri

*Sejak versi 0.1.0.*

1. Buka **Servers** → **Create server** (buat server).
2. Pilih caranya:
   - **Server software** (perangkat lunak server): Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack:** paket yang sama dengan yang dimainkan teman Anda.
   - **From an instance** (dari instalasi): mod yang sudah Anda pakai.
   - **From a file** (dari file).
3. Tekan **Create**. Jendela tertutup dan unduhan berlanjut di kanan bawah.
4. Setujui EULA Minecraft saat diminta, lalu tekan **Start**.

Di halaman server:

- **Console** (konsol): apa yang dikatakan server; ketik perintah di bagian bawah.
- **Players** (pemain): siapa yang sedang online, ditambah whitelist, operator, dan ban.
- **Mods** atau **Plugins:** tambah, perbarui, atau matikan. **Perbarui semua** mengunduh hingga empat sekaligus; selama server berjalan, pembaruan dipasang saat mulai ulang berikutnya.
- **Properties** (properti): pengaturan server (tingkat kesulitan, whitelist nyala/mati, dan sebagainya).
- **Settings** (pengaturan):
  - memori, Java, dan **Java flags** (milik ResinCraft atau milik modpack itu sendiri);
  - cara teman terhubung;
  - ikon server.
- **Mulai ulang** dan menu **⋯** (salin alamat, buka folder, cadangkan sekarang, hapus).

- **Settings → Anti-xray** (pengaturan → anti-xray; Paper, Purpur, Folia, Leaf): sembunyikan bijih dari mod dan texture pack x-ray. **Hide ores** (sembunyikan bijih) ringan untuk server; **Fake ores** (bijih palsu) lebih kuat. Berlaku saat server dimulai berikutnya.
- **Settings → Anti-cheat** (pengaturan → anti-cheat; server plugin): satu sakelar memasang Grim.

Mod yang hanya berjalan di klien game (shader, minimap…) tetap ada di server selama server bisa mulai dengan baik. Jika salah satunya membuat server crash, ResinCraft mematikan mod itu dan memulai server lagi; tab **Mods** bisa menyalakannya kembali.

Jika modpack tidak punya paket server resmi, ResinCraft membangun server dari daftar mod paket itu sendiri: jendela setelah **Create** memberi tahu hal ini, dan server menampilkan lencana **Built by ResinCraft** (dibangun oleh ResinCraft). Beberapa file paket hanya bisa diunduh dari situs web pembuatnya. File-file itu tercantum di bagian atas tab **Console**, dengan **Get file** (ambil file). Unduh lewat browser Anda dan ResinCraft menaruhnya di tempatnya sebelum server dimulai berikutnya, atau tekan **Look again** (cari lagi).

## 9. Ajak teman masuk ke server Anda

*Sejak versi 0.2.0.*

Di **Settings** server → **How friends connect** (cara teman terhubung):

- **Automatic** (disarankan, dan bawaan untuk server baru): router Anda dulu, lalu **ResinCraft Relay**, lalu playit.gg.
- **ResinCraft Relay:** teman bergabung lewat `yourname.rscraft.resincore.tech`. Perlu akun Microsoft yang sudah masuk.
- **playit.gg:** alamat gratis yang tidak berubah. Saat pertama dijalankan, Anda diminta menautkan akun playit.gg (akun tamu juga boleh).
- **Port diteruskan sendiri:** jika Anda mengatur router sendiri.

Tab **Console** menampilkan alamat yang perlu Anda berikan ke teman. Teman yang main Bedrock juga bisa bergabung ke server Java: nyalakan **Bedrock players can join** (pemain Bedrock bisa bergabung).

**Domain milik sendiri:** dengan ResinCraft Relay, nyalakan **Hubungkan domain sendiri ke ResinCraft Relay** dan ketik domainnya. Tambahkan record DNS yang ditampilkan, termasuk record TXT yang membuktikan domain itu milik Anda, lalu tekan **Verifikasi**. Lencananya berubah dari **Menunggu verifikasi** menjadi **Terverifikasi**. Jika DNS otomatis Cloudflare menyala, ResinCraft menambahkan record-nya untuk Anda.

## 10. Permintaan whitelist (halaman web dan bot Discord)

*Sejak versi 1.2.7.*

Biarkan pemain meminta untuk bergabung, lalu Anda sendiri yang menerima atau menolak.

1. Di **Properties**, atur **white-list** ke **true** (kalau tidak, siapa pun tetap bisa bergabung).
2. Buka tab **Players** server dan nyalakan **Whitelist requests** (permintaan whitelist).
3. Pilih cara pemain meminta:
   - **Web page** (halaman web):
     - Berfungsi jika server dibagikan lewat ResinCraft Relay.
     - Tekan **Copy link** (salin tautan) dan kirim tautannya ke pemain. Tautannya adalah alamat server Anda sendiri, atau domain Anda yang terverifikasi.
     - Halaman ini berfungsi selama server online.
   - **Your Discord bot** (bot Discord Anda):
     1. Buka [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token**, lalu salin token-nya.
     2. Tempel di ResinCraft dan tekan **Save** (simpan). Token disimpan aman di PC Anda dan tidak akan pernah ditampilkan lagi.
     3. Saat muncul **Online as …**, tekan **Add the bot to your Discord** (tambahkan bot ke Discord Anda) dan pilih server Discord Anda.
     4. Pemain mengetik `/whitelist TheirName` di Discord Anda.
4. Permintaan muncul di tab **Players**:
   - **Accept** (terima): memasukkan pemain ke whitelist (bot memberi tahu mereka lewat pesan langsung).
   - **Reject** (tolak): menolak permintaan.
   - **Reject and block** (tolak dan blokir): menolaknya dan mencegah orang itu meminta lagi. Pemain yang diblokir muncul di daftar **Blocked** (diblokir), masing-masing dengan **Unblock** (buka blokir).
   - **Dari daftar server:** pemain yang memakai ResinCraft bisa meminta dari halaman server Anda di Jelajahi → Server (bagian 17).

Permintaan dari pemain yang di-ban otomatis ditolak. Bot Discord berjalan selama ResinCraft terbuka; nyalakan system tray (bagian 13) agar bot tetap berjalan saat jendela ditutup.

## 11. Bedrock Edition

*Sejak versi 0.3.1.*

Di Windows, **Bedrock** di sebelah kiri memasang, membuka, dan mencopot Minecraft for Windows dan Minecraft Preview. Sebelum mencopot, dunia Anda dicadangkan lebih dulu.

## 12. Pindah dari launcher lain

*Sejak versi 1.0.5.*

**Pustaka** → **Pindah dari launcher lain** membawa instalasi dari:

- Prism Launcher dan MultiMC
- aplikasi Modrinth dan aplikasi CurseForge
- ATLauncher dan GDLauncher
- Minecraft Launcher resmi

Mod, dunia, dan pengaturannya ikut terbawa. Yang asli tetap di tempatnya.

## 13. Pembaruan dan system tray

*Sejak versi 0.3.0 (system tray sejak 1.2.7).*

- **Pembaruan:**
  - Saat versi baru ResinCraft siap, tombol **Perbarui** muncul di bilah judul.
  - Jika ada game atau server yang berjalan, Anda memilih **Tutup dan perbarui** (server menyimpan dunianya dulu) atau **Perbarui saat berhenti**.
- **Perbarui diam-diam** (Pengaturan → Pembaruan): memasang versi baru di latar belakang, tanpa bertanya, begitu tidak ada yang berjalan.
- **Tetap berjalan di system tray** (Pengaturan → Tampilan):
  - Menutup jendela tidak menghentikan ResinCraft: server, bot Discord, dan unduhan tetap berjalan.
  - Klik ikon tray untuk membukanya lagi; klik kanan → **Quit ResinCraft** (keluar dari ResinCraft) untuk benar-benar keluar.

## 14. Tema dan CSS kustom

*Sejak versi 1.3.0.*

1. Buka **Pengaturan → Tampilan → Tema** dan pilih kartu: **Sistem**, Gelap, Terang, Obsidian, Deepslate, Ceri, Nether, atau The End.
2. **CSS kustom:** tekan **Sisipkan warna saat ini**, ubah warna mana saja, lalu tekan **Terapkan**. Ini berlaku di atas tema apa pun; **Hapus** menghilangkannya.
3. Buka **Cara kustomisasi** di bawah kotak: di sana ada daftar warna dan bentuk yang bisa diubah, bagian-bagian aplikasi yang bisa diberi gaya, dan contoh (gambar latar, sudut lebih bulat, teks lebih besar…). Klik salah satunya untuk menambahkannya ke CSS Anda.

Psst: coba kode Konami (↑ ↑ ↓ ↓ ← → ← → B A), atau klik logo ResinCraft tujuh kali.

## 15. Kendali jarak jauh (konsol web)

*Sejak versi 1.3.0.*

Jalankan server Anda dari browser mana saja, atau dari ponsel.

1. Masuk dengan akun Microsoft di **Akun**.
2. Nyalakan **Pengaturan → Kendali jarak jauh → Izinkan kendali jarak jauh**. Saat muncul **Terhubung sebagai …**, semuanya siap.
3. Nyalakan **Tetap berjalan di system tray** (bagian 13), agar tetap berfungsi setelah jendela ditutup.
4. Buka `https://console.rscraft.resincore.tech` (atau tekan **Buka konsol web**) dan masuk dengan **akun Minecraft yang sama**.

Dari konsol, Anda bisa melakukan semua yang bisa dilakukan pengelola server: memulai, menghentikan, dan memulai ulang server, memantau konsol dan mengetik perintah, mengelola pemain dan permintaan whitelist, menambah dan memperbarui plugin dan mod, mengubah properti dan pengaturan, mengganti versi modpack, menginstal ulang, membuat cadangan, dan membuat server baru. Setiap perubahan muncul sebagai catatan di ResinCraft.

Pengelola file, pengaturan Java, dan plugin dari tautan unduhan mana pun bisa menjalankan program di PC Anda, jadi semuanya memerlukan sakelar kedua: **Izinkan kendali penuh**. Nyalakan hanya jika akun Minecraft (Microsoft) Anda memakai verifikasi dua langkah. Tanpa sakelar itu, konsol tidak akan pernah bisa mengakses apa pun lainnya di PC Anda.

## 16. Instal ulang dan perbaiki

*Sejak versi 1.3.0.*

- **Instalasi tidak mau mulai karena file hilang atau rusak:**
  - Klik kanan instalasi itu di **Pustaka** → **Instal ulang** → **File game**, atau buka **Pengaturan → Perbaiki**-nya.
  - Minecraft, pemuat mod, library, dan aset diunduh lagi. Dunia, mod, dan pengaturan tetap ada.
- **File modpack diubah atau dihapus:** **Instal ulang** → **File modpack** mengembalikan mod, config, dan skrip paket. File yang Anda ubah dicadangkan dulu; dunia dan mod Anda sendiri tetap ada.
- **Server:** menu **⋯**-nya → **Instal ulang** → **Software server** (atau **File modpack** untuk server modpack). Dunia, pengaturan, dan daftar pemain tetap ada.
- **ResinCraft itu sendiri:** **Pengaturan → Pembaruan → Instal ulang ResinCraft** memasang ulang aplikasi di atas dirinya sendiri.

## 17. Cari dan gabung ke server

*Sejak versi 1.4.0.*

1. Buka **Jelajahi** → **Server**. Di sana ada server yang di-host dengan ResinCraft dan server yang ditambahkan pemain.
2. Cari, atau saring menurut kategori (SMP, PvP, Minigame, Skyblock…), wilayah, bahasa, modpack, serta apakah pemain Bedrock atau profil offline bisa bergabung.
3. Klik ikon atau nama server untuk membuka halamannya: deskripsi lengkap, semua detail, paket klien, dan tautannya.
4. Tekan **Gabung**. ResinCraft memilih instalasi yang cocok dengan server, atau bertanya mana yang mau dipakai. Server dengan mod sendiri punya **Pasang & gabung**, yang membuat instalasi dengan mod yang persis sama lalu bergabung.
5. Server ber-whitelist? Tekan **Minta gabung**. Tombolnya menampilkan **Waiting for the host** (menunggu host), lalu **On the whitelist** (sudah di whitelist; tekan **Gabung**) atau **Ask again** (minta lagi). ResinCraft memberi tahu Anda, dan membuat taskbar berkedip, saat host menjawab.

Beri suka pada server yang Anda nikmati, dan laporkan yang melanggar aturan.

## 18. Daftarkan server Anda dan bagikan mod-nya

*Sejak versi 1.4.0.*

1. Di **Settings** (pengaturan) server yang Anda host, nyalakan **Daftar server** (bawaannya mati).
2. Tambahkan deskripsi, tag, wilayah, dan bahasa. Dukungan Bedrock dan profil offline serta modpack-nya terisi sendiri.
3. **Paket klien:** terbitkan apa yang dibutuhkan pemain, agar mereka bisa memakai **Pasang & gabung**:
   - modpack server, dipasang dari situsnya sendiri;
   - setiap mod lain, dari Modrinth atau CurseForge;
   - file Anda sendiri, dikirim langsung dari PC Anda selama ResinCraft berjalan;
   - paket sumber daya dan shader yang Anda pilih untuk ikut dikirim.
   Server plugin dan vanilla juga bisa punya: pilih instalasi dengan versi Minecraft server itu.

Server yang Anda jalankan di tempat lain? Tambahkan dengan **Kirim server** di Jelajahi → Server.

## 19. PC Anda yang lain dan teman yang membantu

*Sejak versi 1.4.1.*

**PC Anda yang lain:** nyalakan kendali jarak jauh di setiap PC (bagian 15) dan masuk dengan akun Minecraft yang sama. Lalu, di **Servers**, buka menu PC di bagian atas dan pilih PC. Dari sini Anda bisa memulai, menghentikan, dan mengelola server-nya, serta memakai **Create server** (buat server) atau **Impor server** di sana. Untuk mengimpor dari folder di PC itu, ketik foldernya; ini memerlukan **Izinkan kendali penuh** di PC itu.

**Teman yang membantu menjalankan server Anda:**

1. Buka **Pengaturan → Kendali jarak jauh → People who can use your servers** (orang yang bisa memakai server Anda).
2. Ketik nama Minecraft mereka dan tekan **Add** (tambah).
3. Pilih apa yang boleh mereka lakukan:
   - **Viewer** (penonton): melihat konsol, pemain, dan pengaturan.
   - **Operator:** juga memulai, menghentikan, dan memulai ulang, mengirim perintah, serta menangani pemain dan permintaan whitelist.
   - **Manager** (pengelola): juga mengubah pengaturan, plugin dan mod, serta modpack.
4. Biarkan **All servers** (semua server), atau pilih server di **Which servers?** (server mana?).

Mereka membuka server Anda dari ResinCraft mereka (menu PC di **Servers**) atau konsol web, selama kendali jarak jauh menyala dan ResinCraft berjalan di PC Anda. File, pengaturan Java, serta membuat atau menghapus server tetap urusan Anda.

## 20. Cadangan dunia

*Sejak versi 1.4.0.*

- **Otomatis:** selama game atau server berjalan, dunianya dicadangkan setiap 30 menit, dan 3 cadangan terbaru per dunia disimpan. Ubah atau matikan di **Pengaturan → Cadangan otomatis**.
- **Pulihkan:** di tab **Dunia** sebuah instalasi, atau **Settings → Backups** (pengaturan → cadangan) untuk server yang Anda host, tekan **Pulihkan** pada sebuah cadangan, buatan Anda sendiri atau otomatis. Dunia yang digantikan dipindahkan ke Recycle Bin, jadi pemulihan bisa dibatalkan. Dunia pemain tunggal juga bisa dikembalikan sebagai salinan.

## 21. Penyimpanan dan pindah ke PC lain

*Sejak versi 1.1.0 (Pindah ke PC lain sejak 1.1.1).*

- **Drive lain:** **Pengaturan → Penyimpanan → Ubah…**, pilih drive atau folder, lalu:
  - **Pindahkan instalasi:** memindahkan instalasi, server, dan file bersama Minecraft ke sana.
  - **Gunakan tanpa memindahkan:** yang baru disimpan di sana; instalasi Anda saat ini tetap di tempatnya dan kembali saat Anda beralih lagi.
- **PC baru:** **Pengaturan → Pindah ke PC lain** menyimpan semuanya ke satu file `.resincraft`. Buka file itu dengan ResinCraft di PC baru.

## 22. Pemecahan masalah

- **Game tidak mau mulai:** lihat jendela **(nama instalasi) crash** (bagian 6), atau tab **Log** instalasi. File hilang atau rusak? Instal ulang (bagian 16).
- **Mod tidak muncul di Jelajahi:** mungkin mod itu tidak tersedia untuk versi Minecraft atau loader Anda. Coba versi lain, atau ganti versi instalasi (bagian 5).
- **Teman tidak bisa bergabung:** periksa alamat di tab **Console** server dan pastikan server menunjukkan **Running** (berjalan). Dengan ResinCraft Relay, pastikan Anda sudah masuk dengan akun Microsoft.
- **Server crash saat dimulai:** ResinCraft mematikan mod khusus klien dengan sendirinya lalu memulainya lagi. Cari baris di **Console** yang menyebutkan mod mana yang dimatikan. Jika disebutkan ada file yang hilang, ambil dari daftar di bagian atas tab **Console**.
- **Masih buntu?** Laporkan masalah di [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) beserta log dari tab **Log**.
