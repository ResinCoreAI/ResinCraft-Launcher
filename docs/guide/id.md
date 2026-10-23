# ResinCraft Launcher: panduan pengguna

Cara melakukan hal-hal yang paling umum di ResinCraft. Nama yang **ditebalkan** adalah yang akan Anda lihat di layar.

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
14. [Pemecahan masalah](#14-pemecahan-masalah)

## 1. Pasang dan masuk

1. Unduh ResinCraft untuk sistem Anda dari [halaman Releases](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** `…_amd64_windows.exe`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb`, atau `.tar.gz`
2. Pasang lalu buka. Windows mungkin bilang penerbitnya belum terverifikasi; di macOS, klik kanan aplikasinya dan pilih **Buka** saat pertama kali.
3. Buka **Akun** → **Masuk dengan Microsoft**, lalu masuk di halaman Microsoft sendiri. Kata sandi Anda tidak pernah melewati ResinCraft.
   - Tidak punya akun Microsoft? Tambahkan **Profil offline** untuk pemain tunggal dan LAN.

## 2. Buat instalasi pertama Anda

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

1. Buka instalasi Anda → tab **Konten** → **Tambah mod**. Atau buka **Jelajahi** dari bilah kiri.
2. Cari, atau jelajahi Modrinth, CurseForge, FTB, dan situs lainnya. Hanya versi yang cocok dengan instalasi Anda yang ditampilkan.
3. Tekan **Pasang**. Mod yang dibutuhkan (dependensi) ditambahkan untuk Anda.
4. Kembali di **Konten**:
   - gunakan sakelar di samping mod untuk menyalakan atau mematikannya;
   - tekan **Periksa pembaruan** untuk memperbarui mod (satu atau semua).

Beberapa pembuat mod di CurseForge tidak mengizinkan launcher mengunduh file mereka. ResinCraft lalu menampilkannya di bawah **Unduh semua**: unduh lewat browser Anda, dan ResinCraft akan menemukannya sendiri di folder Unduhan. Browser Anda menyimpan di tempat lain? Tambahkan folder itu di **Pengaturan → Unduhan → Folder yang dipantau**.

## 4. Pasang modpack

1. Buka **Jelajahi** → **Modpack** (atau **Pustaka** → **Instalasi baru** → **Unduh modpack**).
2. Buka sebuah paket untuk melihat deskripsi, versi, dan **Mod** yang ada di dalamnya.
3. Tekan **Pasang paket**. ResinCraft membuat instalasi baru yang sudah berisi semuanya.
4. Saat paket mendapat pembaruan, ResinCraft akan memberi tahu Anda; Anda juga bisa memilih versi paket lain di halaman instalasi.

## 5. Ganti versi Minecraft atau pemuat mod

1. Di halaman instalasi, klik label versi di bagian atas (misalnya **1.21.4 · Fabric 0.16.10**). Ini membuka **Pengaturan → Versi**.
2. Pilih versi **Minecraft**, **Pemuat mod**, dan versi loader-nya.
3. Tekan **Ubah versi** lalu konfirmasi. Saat game dijalankan berikutnya, yang dibutuhkan akan diunduh.
4. Mod yang dibuat untuk versi lama mungkin tidak berfungsi: buka **Konten** dan tekan **Periksa pembaruan**.

## 6. Saat Minecraft crash

Saat game crash, jendela **(nama instalasi) crash** terbuka. ResinCraft membaca laporan crash dan log, lalu memberi tahu apa yang salah, misalnya:

- *Sodium Extra butuh Sodium, yang belum terpasang* → **Perbaiki** memasang Sodium.
- *citresewn membuat game crash* → **Perbaiki** mematikan mod itu (nyalakan lagi nanti di **Konten**).
- *Ini butuh Java 21 atau yang lebih baru* → **Perbaiki** membiarkan ResinCraft memilih Java yang tepat.
- *Minecraft kehabisan memori* → **Perbaiki** memberinya lebih banyak memori.

Tekan **Perbaiki** di masing-masing, atau **Perbaiki semua dan main**. Tidak ada yang berubah sampai Anda menekan tombol. Jika ResinCraft tidak tahu penyebabnya, **Buka log** menampilkan pesan terakhir dari game.

## 7. Salin dari instalasi lain

1. Buka instalasi *tujuan* salinan → tekan tombol **Salin dari instance lain** di bagian atas.
2. Pilih instalasi *sumber* salinan.
3. Centang yang Anda inginkan:
   - **Dunia**;
   - **Mod** dan **Pengaturan mod** (folder config);
   - **Shader pack** dan **Resource pack**;
   - **Pengaturan game** (video, kontrol, tombol pintas).
4. Tekan **Salin**. Dunia yang namanya sudah dipakai akan mendapat nama baru. Mod dan paket yang sudah Anda punya dibiarkan apa adanya.

## 8. Jalankan server sendiri

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
- **Mods** atau **Plugins:** tambah, perbarui, atau matikan.
- **Properties** (properti): pengaturan server (tingkat kesulitan, whitelist nyala/mati, dan sebagainya).
- **Settings** (pengaturan):
  - memori, Java, dan **Java flags** (milik ResinCraft atau milik modpack itu sendiri);
  - cara teman terhubung;
  - ikon server.
- **Mulai ulang** dan menu **⋯** (salin alamat, buka folder, cadangkan sekarang, hapus).

Mod yang hanya berjalan di klien game (shader, minimap…) otomatis dimatikan di server. Anda bisa melihatnya di tab **Mods** dan menyalakannya lagi jika perlu.

## 9. Ajak teman masuk ke server Anda

Di **Settings** server → **How friends connect** (cara teman terhubung):

- **Automatic** (disarankan): mencoba **ResinCraft Relay** dulu, lalu playit.gg, lalu bore.
- **ResinCraft Relay:** teman bergabung lewat `yourname.rscraft.resincore.tech`. Perlu akun Microsoft yang sudah masuk.
- **playit.gg:** alamat gratis yang tidak berubah. Saat pertama dijalankan, Anda diminta menautkan akun playit.gg (akun tamu juga boleh).
- **Port diteruskan sendiri:** jika Anda mengatur router sendiri.

Tab **Console** menampilkan alamat yang perlu Anda berikan ke teman. Teman yang main Bedrock juga bisa bergabung ke server Java: nyalakan **Bedrock players can join** (pemain Bedrock bisa bergabung).

## 10. Permintaan whitelist (halaman web dan bot Discord)

Biarkan pemain meminta untuk bergabung, lalu Anda sendiri yang menerima atau menolak.

1. Di **Properties**, atur **white-list** ke **true** (kalau tidak, siapa pun tetap bisa bergabung).
2. Buka tab **Players** server dan nyalakan **Whitelist requests** (permintaan whitelist).
3. Pilih cara pemain meminta:
   - **Web page** (halaman web):
     - Berfungsi jika server dibagikan lewat ResinCraft Relay.
     - Tekan **Copy link** (salin tautan) dan kirim tautannya ke pemain: `https://rscraft.resincore.tech/join/<name>`.
     - Halaman ini berfungsi selama server online.
   - **Your Discord bot** (bot Discord Anda):
     1. Buka [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token**, lalu salin token-nya.
     2. Tempel di ResinCraft dan tekan **Save** (simpan). Token disimpan aman di PC Anda dan tidak akan pernah ditampilkan lagi.
     3. Saat muncul **Online as …**, tekan **Add the bot to your Discord** (tambahkan bot ke Discord Anda) dan pilih server Discord Anda.
     4. Pemain mengetik `/whitelist TheirName` di Discord Anda.
4. Permintaan muncul di tab **Players**:
   - **Accept** (terima): memasukkan pemain ke whitelist (bot memberi tahu mereka lewat pesan langsung).
   - **Reject** (tolak): menolak permintaan.
   - **Reject and block** (tolak dan blokir): menolaknya dan mencegah orang itu meminta lagi. **Unblock all** (buka semua blokir) membatalkan blokir Discord.

Permintaan dari pemain yang di-ban otomatis ditolak. Bot Discord berjalan selama ResinCraft terbuka; nyalakan system tray (bagian 13) agar bot tetap berjalan saat jendela ditutup.

## 11. Bedrock Edition

Di Windows, **Bedrock** di sebelah kiri memasang, membuka, dan mencopot Minecraft for Windows dan Minecraft Preview. Sebelum mencopot, dunia Anda dicadangkan lebih dulu.

## 12. Pindah dari launcher lain

**Pustaka** → **Pindah dari launcher lain** membawa instalasi dari:

- Prism Launcher dan MultiMC
- aplikasi Modrinth dan aplikasi CurseForge
- ATLauncher dan GDLauncher
- Minecraft Launcher resmi

Mod, dunia, dan pengaturannya ikut terbawa. Yang asli tetap di tempatnya.

## 13. Pembaruan dan system tray

- **Pembaruan:**
  - Saat versi baru ResinCraft siap, tombol **Perbarui** muncul di bilah judul.
  - Jika ada game atau server yang berjalan, Anda memilih **Tutup dan perbarui** (server menyimpan dunianya dulu) atau **Perbarui saat berhenti**.
- **Perbarui diam-diam** (Pengaturan → Pembaruan): memasang versi baru di latar belakang, tanpa bertanya, begitu tidak ada yang berjalan.
- **Tetap berjalan di system tray** (Pengaturan → Tampilan):
  - Menutup jendela tidak menghentikan ResinCraft: server, bot Discord, dan unduhan tetap berjalan.
  - Klik ikon tray untuk membukanya lagi; klik kanan → **Quit ResinCraft** (keluar dari ResinCraft) untuk benar-benar keluar.

## 14. Pemecahan masalah

- **Game tidak mau mulai:** lihat jendela **(nama instalasi) crash** (bagian 6), atau tab **Log** instalasi.
- **Mod tidak muncul di Jelajahi:** mungkin mod itu tidak tersedia untuk versi Minecraft atau loader Anda. Coba versi lain, atau ganti versi instalasi (bagian 5).
- **Teman tidak bisa bergabung:** periksa alamat di tab **Console** server dan pastikan server menunjukkan **Running** (berjalan). Dengan ResinCraft Relay, pastikan Anda sudah masuk dengan akun Microsoft.
- **Server crash saat dimulai:** ResinCraft mematikan mod khusus klien dengan sendirinya lalu memulainya lagi. Cari baris di **Console** yang menyebutkan mod mana yang dimatikan.
- **Masih buntu?** Laporkan masalah di [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) beserta log dari tab **Log**.
