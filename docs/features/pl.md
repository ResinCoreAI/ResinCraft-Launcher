# ResinCraft Launcher: funkcje

Nowoczesny, przyjazny launcher Minecrafta od **Puirkle**, dla Windowsa, macOS i Linuksa. Bez telemetrii i bez konta ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Pozycje oznaczone **(1.2.7)** są nowe w wersji 1.2.7.

## Granie

- **Instancje:** osobne instalacje dla każdej wersji lub modpacka, każda z własnymi modami, światami, Javą, pamięcią i argumentami JVM.
- **Każda wersja Minecrafta:** wszystkie wydania i snapshoty, aż po klasyczne wersje alfa.
- **Loadery modów:** Fabric, Quilt, Forge (aż do Minecrafta 1.2.5), NeoForge i LiteLoader, z wyborem wersji loadera. Do tego OptiFine i jar mody.
- **Zmiana wersji i loadera (1.2.7):** zmienisz wersję Minecrafta albo loader modów instancji w jej karcie Ustawienia, tak jak w Prism. Otwiera ją też etykieta wersji na stronie instancji.
- **Graj:** jedno kliknięcie. **Zagraj też jako** inne konto albo uruchom tę samą instancję dwa razy na dwóch kontach.
- **Szybkie dołączanie:** twoje ostatnie serwery i światy jednoosobowe, jedno kliknięcie i już jesteś w grze.
- **Światy:** zagraj w świat bezpośrednio, zmień jego nazwę, zrób kopię zapasową, zaimportuj świat z pliku zip.
- **Naprawiacz awarii (1.2.7):** gdy Minecraft się zawiesi, ResinCraft czyta raport awarii i logi. Mówi prostymi słowami, co poszło nie tak, i proponuje naprawę jednym kliknięciem. Naprawia: instaluje brakujący mod, wyłącza mod, który spowodował awarię, wybiera właściwą Javę, daje grze więcej pamięci.
- **Java:** znajduje zainstalowane Javy, dobiera właściwą do każdej wersji Minecrafta i pobiera Eclipse Temurin, gdy żadna nie pasuje.
- **Podgląd RAM:** ile pamięci zużywa launcher, każda gra i każdy serwer (opcjonalnie na ekranie Start).
- **Discord Rich Presence:** nazwa paczki, wersja, czas gry, gra jednoosobowa lub serwer. Przyciski dla znajomych: **View modpack** (zobacz modpack) i **Get ResinCraft** (pobierz ResinCraft).

## Instancje

- **Porządek:** grupy, własna kolejność (przeciągnij, by ułożyć), ikony, banery i kolory.
- **Karty:** Zawartość, Światy, Serwery, Zrzuty ekranu, Notatki, Pliki (wbudowany menedżer i edytor plików), Logi i Ustawienia.
- **Kopiuj z innej instancji:** światy, mody i ich ustawienia (folder config), paczki shaderów, paczki zasobów i ustawienia gry.
- **Duplikowanie, eksport** (Modrinth `.mrpack` lub CurseForge `.zip`) i **tworzenie własnego modpacka**.
- **Polecenia i zmienne środowiskowe:** polecenia uruchamiane przed startem i po zamknięciu gry oraz zmienne środowiskowe.
- **Błyskawiczne przełączniki (1.2.7):** włączenie lub wyłączenie moda działa od razu; zmiana nazwy pliku dzieje się w tle.

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
- **Mody tylko dla klienta wyłączone:** mody, które działają tylko po stronie klienta gry, są wykrywane i wyłączane automatycznie przy każdym starcie serwera. Mod, którego potrzebuje inny mod, zostaje włączony. Jeśli któryś i tak wysypie serwer, zostaje wyłączony, a serwer sam się restartuje.
- **Zarządzanie:** konsola, edytor właściwości, menedżer pluginów i modów z aktualizacjami, whitelista, operatorzy i bany, ikona serwera, kopie zapasowe.
- **Restart i menu „więcej” (1.2.7):** Restart obok Stop; menu z kopiowaniem adresu, otwieraniem folderu, kopią zapasową, ustawieniami i usuwaniem.
- **Teraz online:** gracze na twoim serwerze, sprawdzani na żywo co kilka sekund **(1.2.7)**.
- **Flagi Javy (1.2.7):** dopracowane flagi ResinCraft albo flagi samego modpacka (`user_jvm_args.txt`, skrypty startowe).
- **Granie ze znajomymi:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - twój router (UPnP)
  - twoja własna domena, z automatyzacją DNS w Cloudflare
- **Bedrock players can join** (gracze Bedrock mogą dołączyć) na serwery Java (Geyser + Floodgate).
- **Whitelist requests (1.2.7)** (prośby o whitelistę): gracze proszą o dołączenie, a ty akceptujesz lub odrzucasz ich w karcie Players.
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
- **Aktualizacje:** sprawdzane automatycznie.
  - **(1.2.7)** Przycisk **Aktualizuj** na pasku tytułu.
  - **(1.2.7)** **Zamknij i zaktualizuj**, gdy działają gry lub serwery.
  - **(1.2.7)** **Aktualizuj po cichu** w tle.
- **Działaj w zasobniku systemowym (1.2.7):** po zamknięciu okna serwery, bot Discord i pobieranie dalej działają.
- **16 języków:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Menedżer miejsca na dysku** (przenieś dane na inny dysk), **kopia zapasowa i przywracanie** ustawień, motywy **jasny i ciemny**.
