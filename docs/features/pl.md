# ResinCraft Launcher: funkcje

Nowoczesny, przyjazny launcher Minecrafta od **Puirkle**, dla Windowsa, macOS i Linuksa. Bez telemetrii i bez konta ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Pozycje oznaczone **(nowość)** są nowe w najnowszej wersji.

## Granie

- **Instancje:** osobne instalacje dla każdej wersji lub modpacka, każda z własnymi modami, światami, Javą, pamięcią i argumentami JVM.
- **Każda wersja Minecrafta:** wszystkie wydania i snapshoty, aż po klasyczne wersje alfa.
- **Loadery modów:** Fabric, Quilt, Forge (aż do Minecrafta 1.2.5), NeoForge i LiteLoader, z wyborem wersji loadera. Do tego OptiFine i jar mody.
- **Zmiana wersji i loadera:** zmienisz wersję Minecrafta albo loader modów instancji w jej karcie Ustawienia, tak jak w Prism. Otwiera ją też etykieta wersji na stronie instancji.
- **Graj:** jedno kliknięcie. **Zagraj też jako** inne konto albo uruchom tę samą instancję dwa razy na dwóch kontach.
- **Szybkie dołączanie:** twoje ostatnie serwery i światy jednoosobowe, jedno kliknięcie i już jesteś w grze.
- **Światy:** zagraj w świat bezpośrednio, zmień jego nazwę, zrób kopię zapasową, zaimportuj świat z pliku zip.
- **Naprawiacz awarii:** gdy Minecraft się zawiesi, ResinCraft czyta raport awarii i logi. Mówi prostymi słowami, co poszło nie tak, i proponuje naprawę jednym kliknięciem. Naprawia: instaluje brakujący mod, wyłącza mod, który spowodował awarię, wybiera właściwą Javę, daje grze więcej pamięci.
- **Ponowna instalacja (nowość):** pobierz ponownie pliki gry instancji albo przywróć pliki modpacka tak, jak ma je paczka. Z menu instancji lub w **Ustawienia → Naprawa**. Światy zostają.
- **Dopracowane flagi Javy:** gry startują z flagami G1GC od ResinCraft, dla płynniejszych klatek (Ustawienia → Java albo osobno dla instancji).
- **Java:** znajduje zainstalowane Javy, dobiera właściwą do każdej wersji Minecrafta i pobiera Eclipse Temurin, gdy żadna nie pasuje.
- **Podgląd RAM:** ile pamięci zużywa launcher, każda gra i każdy serwer (opcjonalnie na ekranie Start).
- **Discord Rich Presence:** nazwa paczki, wersja, czas gry, gra jednoosobowa lub serwer. Przyciski dla znajomych: **View modpack** (zobacz modpack) i **Get ResinCraft** (pobierz ResinCraft).

## Instancje

- **Porządek:** grupy, własna kolejność (przeciągnij, by ułożyć), ikony, banery i kolory.
- **Karty:** Zawartość, Światy, Serwery, Zrzuty ekranu, Notatki, Pliki (wbudowany menedżer i edytor plików), Logi i Ustawienia.
- **Kopiuj z innej instancji:** światy, mody i ich ustawienia (folder config), paczki shaderów, paczki zasobów i ustawienia gry.
- **Duplikowanie, eksport** (Modrinth `.mrpack` lub CurseForge `.zip`) i **tworzenie własnego modpacka**.
- **Polecenia i zmienne środowiskowe:** polecenia uruchamiane przed startem i po zamknięciu gry oraz zmienne środowiskowe.
- **Błyskawiczne przełączniki:** włączenie lub wyłączenie moda działa od razu; zmiana nazwy pliku dzieje się w tle.
- **Błyskawiczne usuwanie i tworzenie (nowość):** usunięta instancja znika od razu i w tle trafia do Kosza. Instalowany modpack od razu pojawia się w Bibliotece, z postępem na karcie.

## Zawartość i Odkrywaj

- **Serwisy:** mody, modpacki, paczki zasobów i shadery z Modrinth, CurseForge, FTB, ATLauncher, Technic i wydań na GitHub, filtrowane pod wersję i loader instancji.
- **Strony modpacków** z opisem, galerią, wersjami, listami zmian i **modami, które zawiera paczka**.
- **Sprytna instalacja:** wybierz dowolną wersję, zainstaluj kilka naraz, wymagane zależności dodają się same, a ten sam mod z dwóch serwisów zostaje wychwycony.
- **Aktualizacje:** sprawdzanie i instalowanie aktualizacji (jednej lub wszystkich); aktualizacje modpacków i zmiana wersji.
- **Ręczne pobieranie:** pliki z CurseForge, których autorzy blokują launchery, są wyłapywane z folderu Pobrane lub innych **Obserwowanych folderów** (Ustawienia → Pobieranie).
- **Listy:** wyeksportuj mody, paczki zasobów lub shadery jako listę i zainstaluj listę w innej instancji.

## Hostowanie serwerów

- **Typy serwerów:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mody i pluginy razem) oraz Bedrock Dedicated Server.
- **Z modpacka lub instancji:** utwórz serwer z Modrinth, CurseForge (z oficjalną paczką serwerową, jeśli istnieje) lub FTB albo z jednej ze swoich instancji.
- **Brak oficjalnej paczki serwerowej? (nowość):** ResinCraft buduje serwer z listy modów samej paczki, mówi o tym przy tworzeniu serwera i pokazuje plakietkę **Built by ResinCraft** (zbudowane przez ResinCraft).
- **Pliki do samodzielnego pobrania (nowość):** pliki, których autorzy pozwalają pobierać je tylko ze swojej strony, są wypisane w karcie **Console** (konsola) serwera. Przed każdym startem ResinCraft szuka ich w folderze Pobrane i w twoich instancjach tej samej paczki, a potem umieszcza je na miejscu.
- **Pamięć według rozmiaru paczki (nowość):** nowy serwer z modami dostaje 6 GB od 80 modów i 8 GB od 150 (najwyżej połowę RAM-u twojego PC).
- **Mody tylko dla klienta wyłączane dopiero, gdy wysypią serwer:** mody klienckie zostają na serwerze, dopóki ten startuje bez problemu. Jeśli start się wysypie, bo mod sięgnął po klienta gry, ten mod zostaje wyłączony (`.jar.disabled`), a serwer sam startuje ponownie. Mod, którego potrzebuje inny mod, zostaje włączony.
- **Zarządzanie:** konsola, edytor właściwości, menedżer pluginów i modów z aktualizacjami, whitelista, operatorzy i bany, ikona serwera, kopie zapasowe.
- **Restart i menu „więcej”:** Restart obok Stop; menu z kopiowaniem adresu, otwieraniem folderu, kopią zapasową, ustawieniami i usuwaniem.
- **Teraz online:** gracze na twoim serwerze, sprawdzani na żywo co kilka sekund.
- **Flagi Javy:** dopracowane flagi ResinCraft albo flagi samego modpacka (`user_jvm_args.txt`, skrypty startowe).
- **Anti-xray (nowość)** (ochrona przed x-rayem): serwery Paper, Purpur, Folia i Leaf mogą ukrywać rudy przed modami i paczkami tekstur typu x-ray (Settings → Anti-xray).
- **Ponowna instalacja (nowość):** zainstaluj ponownie oprogramowanie serwera albo przywróć pliki jego modpacka. Świat i ustawienia zostają.
- **Błyskawiczne usuwanie i tworzenie (nowość):** usunięty serwer znika od razu; tworzony serwer od razu pojawia się na liście, z postępem.
- **Zdalne sterowanie (nowość):** steruj serwerami z dowolnej przeglądarki lub telefonu przez konsolę webową pod `console.rscraft.resincore.tech`, zalogowany tym samym kontem Minecraft, gdy ResinCraft jest otwarty lub w zasobniku systemowym (Ustawienia → Zdalne sterowanie). Potrafi wszystko, co menedżer serwerów. Menedżer plików, ustawienia Javy i pluginy z dowolnego linku wymagają drugiego przełącznika, **Zezwól na pełną kontrolę**.
- **Granie ze znajomymi:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - twój router (UPnP)
  - twoja własna domena, z automatyzacją DNS w Cloudflare
- **Bedrock players can join** (gracze Bedrock mogą dołączyć) na serwery Java (Geyser + Floodgate).
- **Whitelist requests** (prośby o whitelistę): gracze proszą o dołączenie, a ty akceptujesz lub odrzucasz ich w karcie Players.
  - **Strona WWW:** `https://rscraft.resincore.tech/join/<name>` (z ResinCraft Relay).
  - **Twój własny bot Discord:** gracze wpisują `/whitelist`.
  - **Twoja odpowiedź:** akceptacja dodaje ich do whitelisty; bot daje im znać w wiadomości prywatnej.
  - **Ochrona:** zbanowani gracze są odrzucani; **Reject and block** (odrzuć i zablokuj) sprawia, że ktoś nie może poprosić ponownie.

## Bedrock Edition (Windows)

- Instaluj, uruchamiaj i odinstalowuj Minecraft for Windows i Minecraft Preview (najpierw robiona jest kopia zapasowa światów).

## Konta

- **Logowanie przez Microsoft**, z tokenami przechowywanymi w formie zaszyfrowanej. **Profile offline** do gry jednoosobowej i w sieci LAN.
- **Skiny, peleryny i zmiana nicku.**

## Przenosiny z innego launchera

- Importuj instancje z Prism Launcher, MultiMC, aplikacji Modrinth, aplikacji CurseForge, ATLauncher, GDLauncher i oficjalnego Minecraft Launchera, razem z ich modami, światami, paczkami i ustawieniami. Serwery też da się zaimportować.

## Aplikacja

- **Windows, macOS** (Apple Silicon i Intel) **i Linux** (x64 i ARM64: AppImage, `.deb`, `.tar.gz`).
- **Instalatory dla Windowsa:** plik `.exe`, `.msi` do instalacji zarządzanych i **ResinCraft Setup (nowość)**: mały instalator, który zawsze instaluje najnowszą wersję, pod linkiem, który się nie zmienia.
- **Aktualizacje:** sprawdzane automatycznie.
  - Przycisk **Aktualizuj** na pasku tytułu.
  - **Zamknij i zaktualizuj**, gdy działają gry lub serwery.
  - **Aktualizuj po cichu** w tle.
- **Działaj w zasobniku systemowym:** po zamknięciu okna serwery, bot Discord, zdalne sterowanie i pobieranie dalej działają.
- **Ponowna instalacja ResinCraft (nowość):** Ustawienia → Aktualizacje instaluje aplikację ponownie na sobie, żeby ją naprawić. Instancje, konta i ustawienia zostają.
- **Motywy (nowość):** Ciemny, Jasny, Obsydian, Łupek, Wiśnia, Nether i Kres albo Systemowy.
  - **Własny CSS:** twój własny CSS na dowolnym motywie, zaczynając od obecnych kolorów, z wbudowanym przewodnikiem po kolorach, częściach aplikacji i gotowych przykładach.
  - I ukryta niespodzianka. Spróbuj kodu Konami.
- **16 języków:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Menedżer miejsca na dysku** (przenieś dane na inny dysk), **kopia zapasowa i przywracanie** ustawień.
