# ResinCraft Launcher: özellikler

**Puirkle** imzalı, Windows, macOS ve Linux için modern ve kullanışlı bir Minecraft başlatıcısı. Telemetri yok, ResinCraft hesabı da yok.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

**(1.2.7)** ile işaretlenen maddeler 1.2.7 sürümünde yeni.

## Oyun

- **Kurulumlar:** her sürüm veya mod paketi için ayrı kurulumlar; her birinin kendi modları, dünyaları, Java'sı, belleği ve JVM argümanları var.
- **Her Minecraft sürümü:** tüm sürümler ve snapshot'lar, klasik alfa sürümlerine kadar.
- **Mod yükleyiciler:** Fabric, Quilt, Forge (Minecraft 1.2.5'e kadar), NeoForge ve LiteLoader, yükleyici sürümü seçimiyle birlikte. OptiFine ve jar modları da var.
- **Sürümü ve yükleyiciyi değiştir (1.2.7):** bir kurulumun Minecraft sürümünü veya mod yükleyicisini, Prism'deki gibi Ayarlar sekmesinden değiştir. Kurulum sayfasındaki sürüm etiketi de burayı açar.
- **Oyna:** tek tık. **Şu hesapla da oyna** ile başka bir hesapla oyna ya da aynı kurulumu iki hesapla iki kez çalıştır.
- **Hızlı katıl:** son sunucuların ve tek oyunculu dünyaların, tek tıkla doğrudan oyuna gir.
- **Dünyalar:** bir dünyayı doğrudan oyna, adını değiştir, yedekle, zip dosyasından dünya içe aktar.
- **Çökme düzeltici (1.2.7):** Minecraft çöktüğünde ResinCraft çökme raporunu ve günlükleri okur. Neyin ters gittiğini sade bir dille anlatır ve tek tıkla düzeltme önerir. Düzeltmeler: eksik bir modu yükler, çökmeye yol açan modu kapatır, doğru Java'yı seçer, oyuna daha fazla bellek verir.
- **Java:** yüklü Java'ları bulur, her Minecraft sürümü için doğru olanı seçer ve hiçbiri uymazsa Eclipse Temurin'i indirir.
- **RAM göstergesi:** başlatıcının, her oyunun ve her sunucunun ne kadar bellek kullandığı (istersen Ana sayfada da).
- **Discord Rich Presence:** paket adı, sürüm, oyun süresi, tek oyunculu ya da sunucu. Arkadaşların için butonlar: **View modpack** (mod paketini gör) ve **Get ResinCraft** (ResinCraft'ı edin).

## Kurulumlar

- **Düzenle:** gruplar, kendi sıran (sürükleyip diz), simgeler, afişler ve renkler.
- **Sekmeler:** İçerik, Dünyalar, Sunucular, Ekran görüntüleri, Notlar, Dosyalar (yerleşik dosya yöneticisi ve düzenleyici), Günlükler ve Ayarlar.
- **Başka bir örnekten kopyala:** dünyalar, modlar ve ayarları (config klasörü), gölgelendirici paketleri, kaynak paketleri ve oyun ayarları.
- **Çoğalt, dışa aktar** (Modrinth `.mrpack` veya CurseForge `.zip`) ve **kendi mod paketine dönüştür**.
- **Komutlar ve ortam:** oyun başlamadan önce ve kapandıktan sonra çalışacak komutlar, ayrıca ortam değişkenleri.
- **Anında açma/kapatma (1.2.7):** bir modu açmak veya kapatmak anında olur; yeniden adlandırma arka planda yapılır.

## İçerik ve Keşfet

- **Siteler:** Modrinth, CurseForge, FTB, ATLauncher, Technic ve GitHub sürümlerinden modlar, mod paketleri, kaynak paketleri ve gölgelendiriciler; kurulumun sürümüne ve yükleyicisine göre süzülür.
- **Mod paketi sayfaları:** açıklama, galeri, sürümler, değişiklik notları ve **paketin içindeki modlar**.
- **Akıllı yükleme:** istediğin sürümü seç, birkaçını aynı anda yükle; gerekli bağımlılıklar kendiliğinden eklenir, iki farklı siteden gelen aynı mod yakalanır.
- **Güncellemeler:** güncellemeleri denetle ve uygula (tek tek veya hepsini); mod paketi güncellemeleri ve sürüm değiştirme.
- **Elle indirmeler:** yazarı başlatıcıları engelleyen CurseForge dosyaları, İndirilenler klasöründen veya diğer **İzlenen klasörler**den alınır (Ayarlar → İndirmeler).
- **Listeler:** modları, kaynak paketlerini veya gölgelendiricileri liste olarak dışa aktar, bir listeyi başka bir kuruluma yükle.

## Sunucu barındırma

- **Sunucu türleri:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mod ve eklentiler bir arada) ve Bedrock Dedicated Server.
- **Bir mod paketinden veya kurulumdan:** Modrinth, CurseForge (varsa resmi sunucu paketiyle) veya FTB'den ya da kendi kurulumlarından birinden sunucu oluştur.
- **Yalnızca istemci modları kapalı kalır:** sadece oyun istemcisinde çalışan modlar, sunucu her başladığında bulunur ve otomatik olarak kapatılır. Başka bir modun ihtiyaç duyduğu mod açık kalır. Yine de biri sunucuyu çökertirse kapatılır ve sunucu kendiliğinden yeniden başlar.
- **Yönet:** konsol, özellik düzenleyici, güncellemeli eklenti ve mod yöneticisi, beyaz liste, op'lar ve yasaklar, sunucu simgesi, yedekler.
- **Yeniden başlat ve daha fazla menüsü (1.2.7):** Durdur'un yanında Yeniden başlat; adresi kopyala, klasörü aç, yedekle, ayarlar ve sil seçenekleri olan bir menü.
- **Şu an çevrimiçi:** sunucundaki oyuncular, birkaç saniyede bir canlı olarak denetlenir **(1.2.7)**.
- **Java bayrakları (1.2.7):** ResinCraft'ın ayarlanmış bayrakları ya da mod paketinin kendi bayrakları (`user_jvm_args.txt`, başlatma betikleri).
- **Arkadaşlarınla paylaşma:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - modemin (UPnP)
  - Cloudflare DNS otomasyonuyla kendi alan adın
- **Bedrock players can join** (Bedrock oyuncuları katılabilir): Java sunucularına (Geyser + Floodgate).
- **Whitelist requests (1.2.7)** (beyaz liste istekleri): oyuncular katılmak için istek gönderir, sen de Players sekmesinde kabul eder ya da reddedersin.
  - **Web sayfası:** `https://rscraft.resincore.tech/join/<name>` (ResinCraft Relay ile).
  - **Kendi Discord botun:** oyuncular `/whitelist` yazar.
  - **Senin cevabın:** kabul edersen beyaz listeye eklenirler; bot bunu onlara özel mesajla bildirir.
  - **Koruma:** yasaklı oyuncular reddedilir; **Reject and block** (reddet ve engelle) birinin tekrar istek göndermesini önler.

## Bedrock Edition (Windows)

- Minecraft for Windows ve Minecraft Preview'u yükle, başlat ve kaldır (önce dünyalar yedeklenir).

## Hesaplar

- **Microsoft ile giriş**, oturum anahtarları şifreli saklanır. Tek oyunculu ve LAN için **çevrimdışı profiller**.
- **Görünümler, pelerinler ve isim değiştirme.**

## Başka bir başlatıcıdan geçiş

- Prism Launcher, MultiMC, Modrinth uygulaması, CurseForge uygulaması, ATLauncher, GDLauncher ve resmi Minecraft Launcher'dan kurulumları modları, dünyaları, paketleri ve ayarlarıyla birlikte içe aktar. Sunucular da içe aktarılabilir.

## Uygulama

- **Windows, macOS** (Apple Silicon ve Intel) **ve Linux** (x64 ve ARM64: AppImage, `.deb`, `.tar.gz`).
- **Güncellemeler:** otomatik olarak denetlenir.
  - **(1.2.7)** Başlık çubuğunda bir **Güncelle** butonu.
  - **(1.2.7)** Oyunlar veya sunucular çalışırken **Kapat ve güncelle**.
  - **(1.2.7)** Arka planda **Sessiz güncelle**.
- **Sistem tepsisinde çalışmaya devam et (1.2.7):** pencereyi kapatınca sunucular, Discord botu ve indirmeler çalışmaya devam eder.
- **16 dil:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Depolama yöneticisi** (verileri başka bir sürücüye taşı), ayarların **yedeklenmesi ve geri yüklenmesi**, **açık ve koyu** temalar.
