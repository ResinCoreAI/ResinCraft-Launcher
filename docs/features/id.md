# ResinCraft Launcher: fitur

Launcher Minecraft yang modern dan ramah buatan **Puirkle**, untuk Windows, macOS, dan Linux. Tanpa telemetri dan tanpa akun ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Item bertanda **(baru)** adalah fitur baru di versi terbaru.

## Bermain

- **Instalasi:** instalasi terpisah untuk setiap versi atau modpack, masing-masing dengan mod, dunia, Java, memori, dan argumen JVM sendiri.
- **Semua versi Minecraft:** setiap rilis dan snapshot, sampai ke versi alpha klasik.
- **Pemuat mod:** Fabric, Quilt, Forge (sampai Minecraft 1.2.5), NeoForge, dan LiteLoader, dengan pemilih versi loader. OptiFine dan jar mod juga bisa.
- **Ganti versi dan loader:** ganti versi Minecraft atau pemuat mod sebuah instalasi dari tab Pengaturan-nya, seperti di Prism. Label versi di halaman instalasi juga membukanya.
- **Main:** sekali klik. **Main juga sebagai** akun lain, atau jalankan instalasi yang sama dua kali dengan dua akun.
- **Gabung cepat:** server terakhir dan dunia pemain tunggal Anda, sekali klik untuk langsung masuk.
- **Dunia:** mainkan dunia secara langsung, ganti namanya, cadangkan, impor dunia dari file zip.
- **Perbaikan crash:** saat Minecraft crash, ResinCraft membaca laporan crash dan log. Ia menjelaskan masalahnya dengan bahasa sederhana dan menawarkan perbaikan sekali klik. Perbaikannya: memasang mod yang kurang, mematikan mod yang menyebabkan crash, memilih Java yang tepat, memberi game lebih banyak memori.
- **Instal ulang (baru):** unduh lagi file game sebuah instalasi, atau kembalikan file modpack seperti aslinya di paket. Dari menu instalasi atau **Pengaturan → Perbaiki**. Dunia tetap aman.
- **Flag Java racikan:** game dimulai dengan flag G1GC racikan ResinCraft agar frame lebih mulus (Pengaturan → Java, atau per instalasi).
- **Java:** menemukan Java yang terpasang, memilih yang tepat untuk tiap versi Minecraft, dan mengunduh Eclipse Temurin jika tidak ada yang cocok.
- **Tampilan RAM:** berapa banyak memori yang dipakai launcher, tiap game, dan tiap server (bisa juga ditampilkan di Beranda).
- **Discord Rich Presence:** nama paket, versi, waktu bermain, pemain tunggal atau server. Tombol untuk teman: **View modpack** (lihat modpack) dan **Get ResinCraft** (dapatkan ResinCraft).

## Instalasi

- **Atur:** grup, urutan Anda sendiri (seret untuk menyusun), ikon, banner, dan warna.
- **Tab:** Konten, Dunia, Server, Tangkapan layar, Catatan, File (pengelola dan editor file bawaan), Log, dan Pengaturan.
- **Salin dari instance lain:** dunia, mod beserta pengaturannya (folder config), shader pack, resource pack, dan pengaturan game.
- **Duplikat, ekspor** (Modrinth `.mrpack` atau CurseForge `.zip`) dan **jadikan modpack Anda sendiri**.
- **Perintah dan lingkungan:** perintah yang dijalankan sebelum game dimulai dan setelah game ditutup, serta variabel lingkungan.
- **Sakelar instan:** menyalakan atau mematikan mod langsung terjadi; penggantian nama file berjalan di latar belakang.
- **Hapus dan buat instan (baru):** instalasi yang dihapus langsung hilang dan dipindahkan ke Recycle Bin di latar belakang. Modpack yang sedang dipasang langsung muncul di Pustaka, dengan progresnya di kartu.

## Konten dan Jelajahi

- **Situs:** mod, modpack, paket sumber daya, dan shader dari Modrinth, CurseForge, FTB, ATLauncher, Technic, dan rilis GitHub, disaring sesuai versi dan loader instalasi.
- **Halaman modpack** dengan deskripsi, galeri, versi, catatan perubahan, dan **daftar mod di dalam paket**.
- **Pemasangan pintar:** pilih versi mana saja, pasang beberapa sekaligus, dependensi wajib ditambahkan otomatis, dan mod yang sama dari dua situs akan terdeteksi.
- **Pembaruan:** periksa dan terapkan pembaruan (satu atau semua); pembaruan modpack dan ganti versi.
- **Unduhan manual:** file CurseForge yang pembuatnya memblokir launcher diambil dari folder Unduhan Anda atau **Folder yang dipantau** lainnya (Pengaturan → Unduhan).
- **Daftar:** ekspor mod, paket sumber daya, atau shader sebagai daftar, lalu pasang daftar itu ke instalasi lain.

## Menjalankan server

- **Jenis server:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mod dan plugin sekaligus), dan Bedrock Dedicated Server.
- **Dari modpack atau instalasi:** buat server dari Modrinth, CurseForge (paket server resmi jika ada), atau FTB, atau dari salah satu instalasi Anda.
- **Tidak ada paket server resmi? (baru):** ResinCraft membangun server dari daftar mod paket itu sendiri, memberi tahu Anda saat server dibuat, dan menampilkan lencana **Built by ResinCraft** (dibangun oleh ResinCraft).
- **File yang perlu diunduh sendiri (baru):** file yang pembuatnya hanya mengizinkan unduhan dari situs web mereka dicantumkan di tab **Console** (konsol) server. Sebelum setiap mulai, ResinCraft mencarinya di folder Unduhan Anda dan di instalasi Anda dari paket yang sama, lalu menaruhnya di tempatnya.
- **Memori sesuai ukuran paket (baru):** server modded baru mendapat 6 GB mulai dari 80 mod dan 8 GB mulai dari 150 mod (paling banyak setengah RAM PC Anda).
- **Mod khusus klien hanya dimatikan saat crash:** mod klien tetap ada di server selama server bisa mulai dengan baik. Jika server crash saat mulai karena sebuah mod mencoba memakai klien game, mod itu dimatikan (`.jar.disabled`) dan server mulai ulang sendiri. Mod yang dibutuhkan mod lain tetap menyala.
- **Kelola:** konsol, editor properti, pengelola plugin dan mod dengan pembaruan, whitelist, op dan ban, ikon server, cadangan.
- **Mulai ulang dan menu lainnya:** Mulai ulang di sebelah Stop; menu berisi salin alamat, buka folder, cadangkan, pengaturan, dan hapus.
- **Sedang online:** pemain di server Anda, diperiksa langsung setiap beberapa detik.
- **Flag Java:** flag racikan ResinCraft, atau milik modpack itu sendiri (`user_jvm_args.txt`, skrip mulai).
- **Anti-xray (baru):** server Paper, Purpur, Folia, dan Leaf bisa menyembunyikan bijih dari mod dan texture pack x-ray (Settings → Anti-xray).
- **Instal ulang (baru):** pasang ulang software server, atau kembalikan file modpack-nya. Dunia dan pengaturan tetap ada.
- **Hapus dan buat instan (baru):** server yang dihapus langsung hilang; server yang sedang dibuat langsung muncul di daftar, dengan progresnya.
- **Kendali jarak jauh (baru):** jalankan server Anda dari browser mana saja atau dari ponsel dengan konsol web di `console.rscraft.resincore.tech`, masuk dengan akun Minecraft yang sama, selama ResinCraft terbuka atau ada di system tray (Pengaturan → Kendali jarak jauh). Konsol ini bisa memulai, menghentikan, memantau, dan membuat server, tetapi tidak pernah bisa mengakses file atau pengaturan Anda yang lain.
- **Berbagi dengan teman:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - router Anda (UPnP)
  - domain Anda sendiri, dengan otomatisasi DNS Cloudflare
- **Bedrock players can join** (pemain Bedrock bisa bergabung) ke server Java (Geyser + Floodgate).
- **Whitelist requests** (permintaan whitelist): pemain meminta untuk bergabung, lalu Anda menerima atau menolaknya di tab Players.
  - **Halaman web:** `https://rscraft.resincore.tech/join/<name>` (dengan ResinCraft Relay).
  - **Bot Discord Anda sendiri:** pemain mengetik `/whitelist`.
  - **Jawaban Anda:** menerima memasukkan mereka ke whitelist; bot memberi tahu mereka lewat pesan langsung.
  - **Perlindungan:** pemain yang di-ban otomatis ditolak; **Reject and block** (tolak dan blokir) mencegah seseorang meminta lagi.

## Bedrock Edition (Windows)

- Pasang, jalankan, dan copot Minecraft for Windows dan Minecraft Preview (dunia dicadangkan lebih dulu).

## Akun

- **Masuk dengan Microsoft**, dengan token yang disimpan terenkripsi. **Profil offline** untuk pemain tunggal dan LAN.
- **Skin, jubah, dan ganti nama.**

## Pindah dari launcher lain

- Impor instalasi dari Prism Launcher, MultiMC, aplikasi Modrinth, aplikasi CurseForge, ATLauncher, GDLauncher, dan Minecraft Launcher resmi, lengkap dengan mod, dunia, paket, dan pengaturannya. Server juga bisa diimpor.

## Aplikasi

- **Windows, macOS** (Apple Silicon dan Intel) **dan Linux** (x64 dan ARM64: AppImage, `.deb`, `.tar.gz`).
- **Penginstal Windows:** `.exe`, `.msi` untuk instalasi terkelola, dan **ResinCraft Setup (baru)**: penginstal kecil yang selalu memasang versi terbaru, di tautan yang tidak pernah berubah.
- **Pembaruan:** diperiksa otomatis.
  - Tombol **Perbarui** di bilah judul.
  - **Tutup dan perbarui**, saat ada game atau server yang berjalan.
  - **Perbarui diam-diam** di latar belakang.
- **Tetap berjalan di system tray:** menutup jendela tetap membuat server, bot Discord, kendali jarak jauh, dan unduhan berjalan.
- **Instal ulang ResinCraft (baru):** Pengaturan → Pembaruan memasang ulang aplikasi di atas dirinya sendiri untuk memperbaikinya. Instalasi, akun, dan pengaturan tetap ada.
- **Tema (baru):** Gelap, Terang, Obsidian, Deepslate, Ceri, Nether, The End, Halloween, dan Natal, atau Sistem.
  - Halloween aktif sendiri dari 16 sampai 31 Oktober, Natal dari 10 sampai 25 Desember. Pilih tema lain dan tema itu tetap milik Anda; atau matikan tema hari raya.
  - **CSS kustom:** CSS Anda sendiri di atas tema apa pun, dimulai dari warna saat ini.
  - Dan ada kejutan tersembunyi. Coba kode Konami.
- **16 bahasa:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Pengelola penyimpanan** (pindahkan data ke drive lain), **cadangkan dan pulihkan** pengaturan.
