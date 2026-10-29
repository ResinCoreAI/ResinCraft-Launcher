# ResinCraft Launcher: özellikler

**Puirkle** imzalı, Windows, macOS ve Linux için modern ve kullanışlı bir Minecraft başlatıcısı. Telemetri yok, ResinCraft hesabı da yok.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Parantez içindeki sürüm, her özelliğin geldiği sürümdür. En yeniler 1.4.0 ve 1.4.1.

## Oyun

- **Kurulumlar (0.1.0):** her sürüm veya mod paketi için ayrı kurulumlar; her birinin kendi modları, dünyaları, Java'sı, belleği ve JVM argümanları var.
- **Her Minecraft sürümü (0.1.0):** tüm sürümler ve snapshot'lar, klasik alfa sürümlerine kadar.
- **Mod yükleyiciler (0.1.0):** Fabric, Quilt, Forge ve NeoForge, yükleyici sürümü seçimiyle birlikte. 1.7.10 ile 1.12.2 arası için klasik Forge (1.1.2), Minecraft 1.2.5'e kadar Forge ve LiteLoader (1.2.0). OptiFine (1.0.1) ve jar modları (1.2.0) da var.
- **Sürümü ve yükleyiciyi değiştir (1.2.7):** bir kurulumun Minecraft sürümünü veya mod yükleyicisini, Prism'deki gibi Ayarlar sekmesinden değiştir. Kurulum sayfasındaki sürüm etiketi de burayı açar.
- **Oyna (0.1.0):** tek tık. **Şu hesapla da oyna** ile başka bir hesapla oyna ya da aynı kurulumu iki hesapla iki kez çalıştır (1.0.5).
- **Hızlı katıl (1.1.0):** son sunucuların ve tek oyunculu dünyaların (1.2.3), tek tıkla doğrudan oyuna gir.
- **Dünyalar (0.1.0):** bir dünyanın adını değiştir, yedekle, zip dosyasından dünya içe aktar. Bir dünyayı doğrudan oyna (1.2.0).
- **Otomatik dünya yedekleri (1.4.0):** bir oyun veya sunucu çalışırken dünyaları her 30 dakikada bir yedeklenir ve her dünya için en yeni 3 yedek tutulur (Ayarlar → Otomatik yedek).
- **Yedekten geri yükle (1.4.0):** bir dünyayı kendi ya da otomatik herhangi bir yedekten geri getir ya da kopya olarak geri al. Yerine geçilen dünya Geri Dönüşüm Kutusu'na gider.
- **Çökme düzeltici (1.2.7):** Minecraft çöktüğünde ResinCraft çökme raporunu ve günlükleri okur. Neyin ters gittiğini sade bir dille anlatır ve tek tıkla düzeltme önerir. Düzeltmeler: eksik bir modu yükler, çökmeye yol açan modu kapatır, doğru Java'yı seçer, oyuna daha fazla bellek verir.
- **Yeniden kur (1.3.0):** bir kurulumun oyun dosyalarını yeniden indir ya da mod paketinin kendi dosyalarını paketteki haline geri getir. Kurulumun menüsünden veya **Ayarlar → Onar**'dan. Dünyalar kalır.
- **Ayarlanmış Java bayrakları (1.2.7):** oyunlar daha akıcı kareler için ResinCraft'ın G1GC bayraklarıyla başlar (Ayarlar → Java ya da kurulum başına).
- **Java (0.1.0):** yüklü Java'ları bulur, her Minecraft sürümü için doğru olanı seçer ve hiçbiri uymazsa Eclipse Temurin'i indirir.
- **RAM göstergesi (1.2.1):** başlatıcının, her oyunun ve her sunucunun ne kadar bellek kullandığı (istersen Ana sayfada da).
- **Discord Rich Presence (1.2.3):** paket adı, sürüm, oyun süresi, tek oyunculu ya da sunucu. Arkadaşların için butonlar (1.2.6): **View modpack** (mod paketini gör) ve **Get ResinCraft** (ResinCraft'ı edin).

## Kurulumlar

- **Düzenle:** gruplar (0.2.0), kendi sıran (0.3.1, sürükleyip diz), simgeler ve afişler (0.3.0) ve renkler (0.1.0).
- **Sekmeler (0.1.0):** İçerik, Dünyalar, Sunucular, Ekran görüntüleri, Notlar, Dosyalar (yerleşik dosya yöneticisi ve düzenleyici, 0.2.0), Günlükler ve Ayarlar.
- **Başka bir örnekten kopyala (1.2.5):** dünyalar, gölgelendirici paketleri, kaynak paketleri ve oyun ayarları; modlar ve ayarları (config klasörü) 1.2.6'dan beri.
- **Çoğalt (0.1.0), dışa aktar** (Modrinth `.mrpack` veya CurseForge `.zip`, 1.0.5) ve **kendi mod paketine dönüştür (1.1.0)**.
- **Birden fazla seç (1.1.0):** birçok kurulumu aynı anda taşı, sil, çoğalt veya güncelle.
- **Komutlar ve ortam (1.2.0):** oyun başlamadan önce ve kapandıktan sonra çalışacak komutlar, ayrıca ortam değişkenleri.
- **Anında açma/kapatma (1.2.7):** bir modu açmak veya kapatmak anında olur; yeniden adlandırma arka planda yapılır.
- **Anında silme ve oluşturma (1.3.0):** silinen bir kurulum hemen kaybolur ve arka planda Geri Dönüşüm Kutusu'na gider. Yüklenmekte olan bir mod paketi, ilerlemesi kartında görünerek hemen Kitaplık'ta belirir.

## İçerik ve Keşfet

- **Siteler (0.1.0):** Modrinth, CurseForge, FTB, ATLauncher, Technic ve GitHub sürümlerinden (0.3.1) modlar, mod paketleri, kaynak paketleri ve gölgelendiriciler; kurulumun sürümüne ve yükleyicisine göre süzülür.
- **Mod paketi sayfaları (1.2.0):** açıklama, galeri, sürümler, değişiklik notları ve **paketin içindeki modlar (1.2.5)**.
- **Akıllı yükleme (0.3.0):** istediğin sürümü seç, birkaçını aynı anda yükle; gerekli bağımlılıklar kendiliğinden eklenir, iki farklı siteden gelen aynı mod yakalanır.
- **Güncellemeler (0.1.0):** güncellemeleri denetle ve uygula (tek tek veya hepsini); mod paketi güncellemeleri ve sürüm değiştirme (1.1.0). **Tümünü güncelle** aynı anda en fazla dördünü indirir (1.4.1).
- **Elle indirmeler (1.0.1):** yazarı başlatıcıları engelleyen CurseForge dosyaları, İndirilenler klasöründen veya diğer **İzlenen klasörler**den alınır (1.0.5, Ayarlar → İndirmeler).
- **Listeler (0.4.0):** modları, kaynak paketlerini veya gölgelendiricileri liste olarak dışa aktar, bir listeyi başka bir kuruluma yükle.
- **Sunucu bul (1.4.0):** Keşfet → **Sunucular**, ResinCraft ile barındırılan sunucuları ve oyuncuların eklediği sunucuları listeler. Ara; kategoriye, bölgeye, dile, mod paketine, Bedrock veya çevrimdışı profil desteğine göre süz; beğen ve bildir.
- **Her sunucu için bir sayfa (1.4.1):** tam açıklama ve MOTD, her ayrıntı, istemci paketi, bağlantılar ve bir beyaz liste isteği kutusu.
- **Tek tıkla katıl (1.4.0):** **Katıl** uyan kurulumu seçer ya da sorar. **Kur ve katıl** tam olarak sunucunun modlarıyla bir kurulum oluşturur ve katılır.
- **Beyaz listeye girmek için iste (1.4.0):** **Katılma isteği gönder**, Minecraft hesabınla imzalanmış bir istek gönderir. **Waiting for the host** (sunucu sahibi bekleniyor), **On the whitelist** (beyaz listedesin) ya da **Ask again** (tekrar iste) gösterir ve sunucu sahibi cevap verdiğinde ResinCraft sana haber verir (ve görev çubuğunu yanıp söndürür) (1.4.1).

## Sunucu barındırma

- **Sunucu türleri (0.1.0):** Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge ve NeoForge; Leaf, Arclight (mod ve eklentiler bir arada) ve Bedrock Dedicated Server 0.3.1'den beri.
- **Bir mod paketinden veya kurulumdan (0.1.0):** Modrinth, CurseForge (varsa resmi sunucu paketiyle) veya FTB'den ya da kendi kurulumlarından birinden sunucu oluştur.
- **Resmi sunucu paketi yok mu? (1.2.11):** ResinCraft sunucuyu paketin kendi mod listesinden kurar, sunucu oluşturulurken bunu söyler ve bir **Built by ResinCraft** (ResinCraft tarafından kuruldu) rozeti gösterir.
- **Kendin indirmen gereken dosyalar (1.2.9):** yazarlarının yalnızca kendi sitelerinden indirilmesine izin verdiği dosyalar, sunucunun **Console** (konsol) sekmesinde listelenir. ResinCraft her başlatmadan önce onları İndirilenler klasöründe ve aynı paketin kurulumlarında arar ve yerlerine koyar.
- **Paket boyutuna göre bellek (1.2.9):** yeni bir modlu sunucu 80 moddan itibaren 6 GB, 150 moddan itibaren 8 GB alır (en fazla bilgisayarının RAM'inin yarısı).
- **Yalnızca istemci modları, sadece çökertirlerse kapatılır (1.2.8):** istemci modları sunucu sorunsuz başladığı sürece sunucuda kalır. Bir başlatma, bir mod oyun istemcisine uzandığı için çökerse o mod kapatılır (`.jar.disabled`) ve sunucu kendiliğinden yeniden başlar. Başka bir modun ihtiyaç duyduğu mod açık kalır.
- **Yönet (0.1.0):** konsol, özellik düzenleyici, güncellemeli eklenti ve mod yöneticisi (0.3.0), beyaz liste, op'lar ve yasaklar, sunucu simgesi (1.2.5), yedekler.
- **Daha hızlı güncellemeler (1.4.1):** **Tümünü güncelle** aynı anda en fazla dört eklenti veya modu indirir ve hepsini birlikte yerine koyar; sunucu çalışıyorsa bir sonraki yeniden başlatmaya sıraya alır.
- **Sunucunu listele (1.4.0):** Settings → **Sunucu listesi**, sunucunu bir açıklama, etiketler, bölge ve dille Keşfet → Sunucular'a koyar. Başka bir yerde çalıştırdığın sunucular **Sunucu gönder** ile eklenebilir.
- **İstemci paketleri (1.4.0):** oyuncuların katılmak için ihtiyaç duyduklarını yayımla. 1.4.1'den beri: sunucunun mod paketi (kendi sitesinden kurulur), diğer her mod Modrinth veya CurseForge'dan ve kendi dosyaların doğrudan bilgisayarından; kaynak paketleri ve gölgelendiriciler de birlikte; eklenti ve vanilla sunucuları da.
- **Yeniden başlat ve daha fazla menüsü (1.2.7):** Durdur'un yanında Yeniden başlat; adresi kopyala, klasörü aç, yedekle, ayarlar ve sil seçenekleri olan bir menü.
- **Şu an çevrimiçi (1.2.7):** sunucundaki oyuncular, birkaç saniyede bir canlı olarak denetlenir.
- **Java bayrakları (0.2.0):** ResinCraft'ın ayarlanmış bayrakları ya da mod paketinin kendi bayrakları (`user_jvm_args.txt`, başlatma betikleri, 1.2.7).
- **Anti-xray (1.3.0)** (x-ray önleme): Paper, Purpur, Folia ve Leaf sunucuları cevherleri x-ray modlarından ve doku paketlerinden gizleyebilir (sunucunun Settings → Anti-xray bölümü).
- **Anti-cheat (1.4.0)** (hile önleme): tek bir anahtar, eklenti sunucularına Grim hile önleyicisini kurar.
- **Dünyayı yeniden oluştur (1.4.0):** bir sunucuyu yeni bir dünyayla baştan başlat; eskisi Geri Dönüşüm Kutusu'na gider.
- **Yeniden kur (1.3.0):** bir sunucunun yazılımını yeniden kur ya da mod paketi dosyalarını geri getir. Dünya ve ayarlar kalır.
- **Anında silme ve oluşturma (1.3.0):** silinen bir sunucu hemen kaybolur; oluşturulmakta olan bir sunucu, ilerlemesiyle birlikte hemen listede görünür.
- **Uzaktan kontrol (1.3.0):** ResinCraft açıkken ya da sistem tepsisindeyken, sunucularını herhangi bir tarayıcıdan veya telefonundan `console.rscraft.resincore.tech` adresindeki web konsoluyla, aynı Minecraft hesabıyla giriş yaparak yönet (Ayarlar → Uzaktan kontrol). Sunucu yöneticisinin yaptığı her şeyi yapabilir. Dosya yöneticisi, Java ayarları ve herhangi bir bağlantıdan eklentiler için ikinci bir anahtar gerekir: **Tam kontrole izin ver**. Servers sayfasında bir **Web konsolu** butonu (1.3.1).
- **Diğer bilgisayarların (1.4.1):** Servers sekmesindeki bilgisayar menüsü, uzaktan kontrolü açık olan diğer bilgisayarlarındaki sunucuları açar: başlat, durdur, oyuncuları, eklentileri ve ayarları yönet, orada sunucu oluştur veya içe aktar.
- **Arkadaşların yardım etsin (1.4.1):** Ayarlar → Uzaktan kontrol → **People who can use your servers** (sunucularını kullanabilecek kişiler): bir Minecraft hesabına tüm sunucuların ya da seçtiklerin için **Viewer** (izleyici), **Operator** (operatör) veya **Manager** (yönetici) erişimi ver. Dosyalar, Java ayarları ve sunucu oluşturma ya da silme sende kalır.
- **Arkadaşlarınla paylaşma:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`, 0.3.0)
  - playit.gg (0.2.0)
  - bore (0.2.0)
  - modemin (UPnP, 0.2.0)
  - Cloudflare DNS otomasyonuyla (0.2.0) kendi alan adın, bir TXT kaydıyla doğrulanır (**Doğrulama bekleniyor**, **Doğrulandı**, 1.3.1)
  - yeni sunucular için **Automatic** (otomatik, 1.3.2): önce modemin, sonra ResinCraft Relay, sonra playit.gg
- **Bedrock players can join (0.3.1)** (Bedrock oyuncuları katılabilir): Java sunucularına (Geyser + Floodgate).
- **Whitelist requests (1.2.7)** (beyaz liste istekleri): oyuncular katılmak için istek gönderir, sen de Players sekmesinde kabul eder ya da reddedersin.
  - **Web sayfası:** sunucunun kendi adresinde ya da doğrulanmış alan adında (1.3.2, ResinCraft Relay ile).
  - **Kendi Discord botun:** oyuncular `/whitelist` yazar.
  - **Sunucu listesi (1.4.0):** ResinCraft kullanan oyuncular Keşfet → Sunucular'dan istek gönderir.
  - **Senin cevabın:** kabul edersen beyaz listeye eklenirler; bot bunu onlara özel mesajla bildirir.
  - **Koruma:** yasaklı oyuncular reddedilir; **Reject and block** (reddet ve engelle) birinin tekrar istek göndermesini önler; bir **Blocked** (engellenenler) listesi ve her oyuncu için **Unblock** (engeli kaldır) var (1.4.1).

## Bedrock Edition (Windows)

- **Bedrock sayfası (0.3.1):** Minecraft for Windows ve Minecraft Preview'u yükle (1.0.0), güncelle ve onar (1.2.0), başlat ve kaldır (1.2.3) (önce dünyalar yedeklenir).

## Hesaplar

- **Microsoft ile giriş (0.1.0)**, oturum anahtarları şifreli saklanır. Tek oyunculu ve LAN için **çevrimdışı profiller (0.1.0)**. Java Edition'ı olmayan Microsoft hesapları için **Bedrock profilleri (1.0.0)**.
- **Görünümler, pelerinler ve isim değiştirme (0.1.0).** Bir görünüm kitaplığı, oyuncu adından görünümler ve çevrimiçi arama (1.0.3); 3B görünüm görüntüleyici (1.1.0).

## Başka bir başlatıcıdan geçiş

- **İçe aktar (1.0.5):** Prism Launcher, MultiMC, Modrinth uygulaması, CurseForge uygulaması, ATLauncher, GDLauncher ve resmi Minecraft Launcher'dan kurulumları modları, dünyaları, paketleri ve ayarlarıyla birlikte içe aktar. Sunucular da içe aktarılabilir.
- **Başka bir bilgisayara taşı (1.1.1):** Ayarlar → Başka bir bilgisayara taşı her şeyi tek bir `.resincraft` dosyasına kaydeder; onu yeni bilgisayarda aç.

## Uygulama

- **Windows (0.1.0), macOS** (Apple Silicon ve Intel) **ve Linux** (x64 ve ARM64: AppImage, `.deb`, 1.2.0; `.tar.gz`, 1.2.4).
- **Windows yükleyicileri:** `.exe` (0.1.0), yönetilen kurulumlar için bir `.msi` (1.2.10) ve **ResinCraft Setup (1.3.0)**: her zaman en yeni sürümü yükleyen, bağlantısı hiç değişmeyen küçük bir yükleyici.
- **Güncellemeler (0.3.0):** otomatik olarak denetlenir.
  - Başlık çubuğunda bir **Güncelle** butonu (1.2.7).
  - Oyunlar veya sunucular çalışırken **Kapat ve güncelle** (1.2.7).
  - Arka planda **Sessiz güncelle** (1.2.7).
- **Sistem tepsisinde çalışmaya devam et (1.2.7):** pencereyi kapatınca sunucular, Discord botu, uzaktan kontrol ve indirmeler çalışmaya devam eder.
- **ResinCraft'ı yeniden kur (1.3.0):** Ayarlar → Güncellemeler, uygulamayı onarmak için onu kendi üstüne yeniden kurar. Kurulumlar, hesaplar ve ayarlar kalır.
- **Temalar:** Koyu, Açık ve Sistem (0.1.0); Obsidyen, Derin Arduvaz, Kiraz, Nether ve End (1.3.0).
  - **Tema oluştur (1.3.0):** kendi teman; her renk için **İnce ayar** (renk seçiciler) ve uygulamanın bölümleri için CSS (1.3.2).
  - **Temaları paylaş (1.3.2):** resmi ve CSS'iyle birlikte bir `.rctheme` dosyası olarak **Dışa aktar**; **Tema içe aktar** ya da dosyaya çift tıkla (1.4.0).
  - **Özel CSS (1.3.0):** istediğin temanın üstüne kendi CSS'in, şu anki renklerden başlayarak; renkleri, uygulamanın bölümlerini ve hazır örnekleri anlatan yerleşik bir rehberle.
  - Bir de gizli bir sürpriz var (1.3.0). Konami kodunu dene.
- **16 dil (0.1.0):** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Depolama yöneticisi (1.1.0):** kurulumları başka bir sürücüde tut. Oraya **Kurulumları taşı** ya da **Taşımadan kullan** (1.4.1).
- Ayarlar'da **Ayarları sıfırla** ve **ResinCraft'ı kaldır (1.4.0)**.
