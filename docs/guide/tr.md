# ResinCraft Launcher: kullanım kılavuzu

ResinCraft'ta en sık yapılan işler nasıl yapılır. **Kalın** yazılan adlar ekranda göreceğin adlardır.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## İçindekiler

1. [Yükle ve giriş yap](#1-yükle-ve-giriş-yap)
2. [Yeni bir kurulum oluştur](#2-yeni-bir-kurulum-oluştur)
3. [Mod, kaynak paketi ve gölgelendirici ekle](#3-mod-kaynak-paketi-ve-gölgelendirici-ekle)
4. [Mod paketi yükle](#4-mod-paketi-yükle)
5. [Minecraft sürümünü veya mod yükleyiciyi değiştir](#5-minecraft-sürümünü-veya-mod-yükleyiciyi-değiştir)
6. [Minecraft çöktüğünde](#6-minecraft-çöktüğünde)
7. [Başka bir kurulumdan kopyala](#7-başka-bir-kurulumdan-kopyala)
8. [Sunucu barındır](#8-sunucu-barındır)
9. [Arkadaşlarını sunucuna al](#9-arkadaşlarını-sunucuna-al)
10. [Beyaz liste istekleri (web sayfası ve Discord botu)](#10-beyaz-liste-istekleri-web-sayfası-ve-discord-botu)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Başka bir başlatıcıdan taşı](#12-başka-bir-başlatıcıdan-taşı)
13. [Güncellemeler ve sistem tepsisi](#13-güncellemeler-ve-sistem-tepsisi)
14. [Sorun giderme](#14-sorun-giderme)

## 1. Yükle ve giriş yap

1. Sistemine uygun ResinCraft'ı [Releases sayfasından](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest) indir:
   - **Windows:** `…_amd64_windows.exe`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` veya `.tar.gz`
2. Yükle ve aç. Windows yayıncının doğrulanmadığını söyleyebilir; macOS'ta ilk seferde uygulamaya sağ tıklayıp **Aç**'ı seç.
3. **Hesaplar** → **Microsoft ile giriş yap**'ı aç ve Microsoft'un kendi sayfasında giriş yap. Şifren hiçbir zaman ResinCraft'tan geçmez.
   - Microsoft hesabın yok mu? Tek oyunculu ve LAN için bir **Çevrimdışı profil** ekle.

## 2. Yeni bir kurulum oluştur

*Kurulum*, kendi modları, dünyaları ve ayarları olan ayrı bir Minecraft yüklemesidir.

1. **Kitaplık** → **Yeni kurulum**'u aç.
2. Bir **Minecraft sürümü** ve bir **Mod yükleyici** seç:
   - **Vanilla:** mod yok.
   - **Fabric** veya **Quilt:** hafif modlar, performans modları.
   - **Forge** veya **NeoForge:** büyük içerik modları.
3. Bir ad ver ve **Kurulum oluştur**'a bas.
4. **Oyna**'ya bas. İlk açılışta oyun ve doğru Java indirilir; sonraki açılışlar hızlıdır.

İpucu: soldaki **Hızlı katıl**, son sunucularını ve dünyalarını listeler; tek tıkla doğrudan oyuna girersin.

## 3. Mod, kaynak paketi ve gölgelendirici ekle

1. Kurulumunu aç → **İçerik** sekmesi → **Mod ekle**. Ya da sol çubuktan **Keşfet**'i aç.
2. Ara ya da Modrinth, CurseForge, FTB ve diğer sitelere göz at. Yalnızca kurulumuna uyan sürümler gösterilir.
3. **Yükle**'ye bas. Gerekli modlar (bağımlılıklar) senin için eklenir.
4. **İçerik**'e döndüğünde:
   - bir modu açıp kapatmak için yanındaki anahtarı kullan;
   - modları güncellemek için (tek tek veya hepsini) **Güncellemeleri denetle**'ye bas.

Bazı CurseForge yazarları, başlatıcıların dosyalarını indirmesine izin vermez. Bu durumda ResinCraft onları **Tümünü indir** altında listeler: tarayıcında indir, ResinCraft onları İndirilenler klasöründe kendisi bulur. Tarayıcın başka bir yere mi kaydediyor? O klasörü **Ayarlar → İndirmeler → İzlenen klasörler** bölümüne ekle.

## 4. Mod paketi yükle

1. **Keşfet** → **Mod paketleri**'ni aç (ya da **Kitaplık** → **Yeni kurulum** → **Mod paketi indir**).
2. Açıklamasını, sürümlerini ve içindeki **Modlar**'ı görmek için bir paketi aç.
3. **Paketi yükle**'ye bas. ResinCraft içinde her şey hazır olan yeni bir kurulum oluşturur.
4. Paket güncellendiğinde ResinCraft sana haber verir; kurulum sayfasından başka bir paket sürümü de seçebilirsin.

## 5. Minecraft sürümünü veya mod yükleyiciyi değiştir

1. Kurulum sayfasında, üstteki sürüm etiketine tıkla (örneğin **1.21.4 · Fabric 0.16.10**). **Ayarlar → Sürüm** açılır.
2. **Minecraft** sürümünü, **Mod yükleyici**'yi ve yükleyici sürümünü seç.
3. **Sürümü değiştir**'e bas ve onayla. Bir sonraki açılışta gerekenler indirilir.
4. Eski sürüm için yapılmış modlar çalışmayabilir: **İçerik**'e git ve **Güncellemeleri denetle**'ye bas.

## 6. Minecraft çöktüğünde

Oyun çöktüğünde **(kurulum adı) çöktü** penceresi açılır. ResinCraft çökme raporunu ve günlükleri okur ve neyin ters gittiğini söyler, örneğin:

- *Sodium Extra, yüklü olmayan Sodium'a ihtiyaç duyuyor* → **Düzelt**, Sodium'u yükler.
- *citresewn oyunu çökertti* → **Düzelt**, o modu kapatır (sonra **İçerik**'ten tekrar açabilirsin).
- *Bunun için Java 21 veya daha yenisi gerekiyor* → **Düzelt**, doğru Java'yı ResinCraft'ın seçmesini sağlar.
- *Minecraft'ın belleği bitti* → **Düzelt**, ona daha fazla bellek verir.

Her birinde **Düzelt**'e ya da **Hepsini düzelt ve oyna**'ya bas. Sen bir butona basana kadar hiçbir şey değişmez. ResinCraft nedenini bulamazsa **Günlükleri aç**, oyunun en son ne dediğini gösterir.

## 7. Başka bir kurulumdan kopyala

1. *İçine* kopyalamak istediğin kurulumu aç → üstteki **Başka bir örnekten kopyala** butonuna bas.
2. *Nereden* kopyalanacağını seç.
3. İstediklerini işaretle:
   - **Dünyalar**;
   - **Modlar** ve **Mod ayarları** (config klasörü);
   - **Gölgelendirici paketleri** ve **Kaynak paketleri**;
   - **Oyun ayarları** (görüntü, kontroller, tuş atamaları).
4. **Kopyala**'ya bas. Adı zaten kullanılan dünyalara yeni bir ad verilir. Zaten sahip olduğun modlar ve paketler olduğu gibi kalır.

## 8. Sunucu barındır

1. **Servers** → **Create server**'ı (sunucu oluştur) aç.
2. Nasıl olacağını seç:
   - **Server software** (sunucu yazılımı): Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack** (mod paketi): arkadaşlarının oynadığı paketin aynısı.
   - **From an instance** (bir kurulumdan): zaten kullandığın modlar.
   - **From a file** (bir dosyadan).
3. **Create**'e bas. Pencere kapanır ve indirme sağ altta devam eder.
4. Sorulduğunda Minecraft EULA'sını kabul et, sonra **Start**'a bas.

Bir sunucunun sayfasında:

- **Console** (konsol): sunucunun söyledikleri; komutları en alta yaz.
- **Players** (oyuncular): şu an kimlerin çevrimiçi olduğu, ayrıca beyaz liste, operatörler ve yasaklar.
- **Mods** veya **Plugins:** ekle, güncelle ya da kapat.
- **Properties** (özellikler): sunucu ayarları (zorluk, beyaz liste açık/kapalı ve benzerleri).
- **Settings** (ayarlar):
  - bellek, Java ve **Java flags** (ResinCraft'ınkiler ya da mod paketinin kendi bayrakları);
  - arkadaşların nasıl bağlanacağı;
  - sunucu simgesi.
- **Yeniden başlat** ve **⋯** menüsü (adresi kopyala, klasörü aç, şimdi yedekle, sil).

Sadece oyun istemcisinde çalışan modlar (gölgelendiriciler, mini haritalar…) sunucularda otomatik olarak kapatılır. Onları **Mods** sekmesinde görebilir, gerekirse tekrar açabilirsin.

## 9. Arkadaşlarını sunucuna al

Sunucunun **Settings** → **How friends connect** (arkadaşlar nasıl bağlanır) bölümünde:

- **Automatic** (önerilen): önce **ResinCraft Relay**'i, sonra playit.gg'yi, sonra bore'u dener.
- **ResinCraft Relay:** arkadaşların `yourname.rscraft.resincore.tech` adresine katılır. Giriş yapılmış bir Microsoft hesabı gerekir.
- **playit.gg:** hiç değişmeyen ücretsiz bir adres. İlk açılışta bir playit.gg hesabı bağlamanı ister (misafir hesabı da olur).
- **Portu kendim yönlendirdim:** modemini kendin ayarladıysan.

**Console** sekmesi, arkadaşlarına vereceğin adresi gösterir. Bedrock'taki arkadaşların da Java sunucularına katılabilir: **Bedrock players can join**'i (Bedrock oyuncuları katılabilir) aç.

## 10. Beyaz liste istekleri (web sayfası ve Discord botu)

Oyuncular katılmak için istek göndersin, sen de kabul et ya da reddet.

1. **Properties** içinde **white-list** değerini **true** yap (yoksa herkes zaten katılabilir).
2. Sunucunun **Players** sekmesini aç ve **Whitelist requests**'i (beyaz liste istekleri) aç.
3. Oyuncuların nasıl istek göndereceğini seç:
   - **Web page** (web sayfası):
     - Sunucu ResinCraft Relay üzerinden paylaşılıyorsa çalışır.
     - **Copy link**'e (bağlantıyı kopyala) bas ve oyunculara bağlantıyı gönder: `https://rscraft.resincore.tech/join/<name>`.
     - Sayfa, sunucu çevrimiçi olduğu sürece çalışır.
   - **Your Discord bot** (Discord botun):
     1. [Discord Developer Portal](https://discord.com/developers/applications)'ı aç → **New Application** → **Bot** → **Reset Token**, ve token'ı kopyala.
     2. ResinCraft'a yapıştır ve **Save**'e (kaydet) bas. Bilgisayarında güvenle saklanır ve bir daha asla gösterilmez.
     3. **Online as …** yazısını gördüğünde **Add the bot to your Discord**'a (botu Discord'una ekle) bas ve Discord sunucunu seç.
     4. Oyuncular Discord'unda `/whitelist TheirName` yazar.
4. İstekler **Players** sekmesinde görünür:
   - **Accept** (kabul et): oyuncuyu beyaz listeye ekler (bot ona özel mesajla haber verir).
   - **Reject** (reddet): isteği reddeder.
   - **Reject and block** (reddet ve engelle): isteği reddeder ve o kişinin tekrar istek göndermesini önler. **Unblock all** (tüm engelleri kaldır), Discord engellerini geri alır.

Yasaklı oyuncuların istekleri otomatik olarak reddedilir. Discord botu, ResinCraft açık olduğu sürece çalışır; pencere kapalıyken de çalışması için sistem tepsisini aç (bölüm 13).

## 11. Bedrock Edition

Windows'ta soldaki **Bedrock**, Minecraft for Windows ve Minecraft Preview'u yükler, açar ve kaldırır. Kaldırmadan önce dünyalarını yedekler.

## 12. Başka bir başlatıcıdan taşı

**Kitaplık** → **Başka bir başlatıcıdan taşı**, şuralardan kurulumları getirir:

- Prism Launcher ve MultiMC
- Modrinth uygulaması ve CurseForge uygulaması
- ATLauncher ve GDLauncher
- resmi Minecraft Launcher

Modları, dünyaları ve ayarları da onlarla birlikte gelir. Orijinaller oldukları yerde kalır.

## 13. Güncellemeler ve sistem tepsisi

- **Güncellemeler:**
  - Yeni bir ResinCraft sürümü hazır olduğunda başlık çubuğunda bir **Güncelle** butonu belirir.
  - Bir oyun veya sunucu çalışıyorsa **Kapat ve güncelle** (sunucular önce dünyalarını kaydeder) ya da **Durduklarında güncelle** seçeneklerinden birini seçersin.
- **Sessiz güncelle** (Ayarlar → Güncellemeler): hiçbir şey çalışmadığında yeni sürümleri sormadan, arka planda yükler.
- **Sistem tepsisinde çalışmaya devam et** (Ayarlar → Görünüm):
  - Pencereyi kapatınca ResinCraft çalışmaya devam eder: sunucular, Discord botu ve indirmeler sürer.
  - Tekrar açmak için tepsi simgesine tıkla; gerçekten çıkmak için sağ tık → **Quit ResinCraft** (ResinCraft'tan çık).

## 14. Sorun giderme

- **Oyun açılmıyor:** **(kurulum adı) çöktü** penceresine (bölüm 6) ya da kurulumun **Günlükler** sekmesine bak.
- **Bir mod Keşfet'te görünmüyor:** Minecraft sürümün veya yükleyicin için olmayabilir. Başka bir sürüm dene ya da kurulumun sürümünü değiştir (bölüm 5).
- **Arkadaşların katılamıyor:** sunucunun **Console** sekmesindeki adresi ve sunucunun **Running** (çalışıyor) dediğini kontrol et. ResinCraft Relay kullanıyorsan bir Microsoft hesabıyla giriş yaptığından emin ol.
- **Sunucu açılırken çöküyor:** ResinCraft yalnızca istemci modlarını kendisi kapatır ve sunucuyu yeniden başlatır. Hangi modu kapattığını söyleyen satır için **Console**'a bak.
- **Hâlâ çözemedin mi?** Sorunu **Günlükler** sekmesindeki günlükle birlikte [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) üzerinden bildir.
