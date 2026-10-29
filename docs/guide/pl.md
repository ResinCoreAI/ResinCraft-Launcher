# ResinCraft Launcher: poradnik

Jak zrobić w ResinCraft najczęstsze rzeczy. Nazwy **pogrubione** to to, co zobaczysz na ekranie. Każda sekcja podaje, w której wersji się pojawiła.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Spis treści

1. [Instalacja i logowanie](#1-instalacja-i-logowanie)
2. [Twoja pierwsza instancja](#2-twoja-pierwsza-instancja)
3. [Dodawanie modów, paczek zasobów i shaderów](#3-dodawanie-modów-paczek-zasobów-i-shaderów)
4. [Instalacja modpacka](#4-instalacja-modpacka)
5. [Zmiana wersji Minecrafta lub loadera modów](#5-zmiana-wersji-minecrafta-lub-loadera-modów)
6. [Gdy Minecraft się zawiesi](#6-gdy-minecraft-się-zawiesi)
7. [Kopiowanie z innej instancji](#7-kopiowanie-z-innej-instancji)
8. [Hostowanie serwera](#8-hostowanie-serwera)
9. [Jak wpuścić znajomych na serwer](#9-jak-wpuścić-znajomych-na-serwer)
10. [Prośby o whitelistę (strona WWW i bot Discord)](#10-prośby-o-whitelistę-strona-www-i-bot-discord)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Przenosiny z innego launchera](#12-przenosiny-z-innego-launchera)
13. [Aktualizacje i zasobnik systemowy](#13-aktualizacje-i-zasobnik-systemowy)
14. [Motywy i własny CSS](#14-motywy-i-własny-css)
15. [Zdalne sterowanie (konsola webowa)](#15-zdalne-sterowanie-konsola-webowa)
16. [Ponowna instalacja i naprawa](#16-ponowna-instalacja-i-naprawa)
17. [Szukanie serwerów i dołączanie](#17-szukanie-serwerów-i-dołączanie)
18. [Umieść swój serwer na liście i udostępnij jego mody](#18-umieść-swój-serwer-na-liście-i-udostępnij-jego-mody)
19. [Twoje inne komputery i znajomi, którzy pomagają](#19-twoje-inne-komputery-i-znajomi-którzy-pomagają)
20. [Kopie zapasowe światów](#20-kopie-zapasowe-światów)
21. [Miejsce na dysku i przenosiny na inny komputer](#21-miejsce-na-dysku-i-przenosiny-na-inny-komputer)
22. [Rozwiązywanie problemów](#22-rozwiązywanie-problemów)

## 1. Instalacja i logowanie

*Od wersji 0.1.0.*

1. Pobierz ResinCraft dla swojego systemu ze [strony Releases](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** [`ResinCraft-Setup.exe`](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/download/setup/ResinCraft-Setup.exe) (zawsze instaluje najnowszą wersję) albo `…_amd64_windows.exe` / `…_amd64_windows.msi`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` lub `.tar.gz`
2. Zainstaluj i uruchom. Windows może ostrzec, że wydawca jest niezweryfikowany; na macOS za pierwszym razem kliknij aplikację prawym przyciskiem i wybierz **Otwórz**.
3. Otwórz **Konta** → **Zaloguj przez Microsoft** i zaloguj się na stronie Microsoftu. Twoje hasło nigdy nie przechodzi przez ResinCraft.
   - Nie masz konta Microsoft? Dodaj **Profil offline** do gry jednoosobowej i w sieci LAN.

## 2. Twoja pierwsza instancja

*Od wersji 0.1.0.*

*Instancja* to osobna instalacja Minecrafta z własnymi modami, światami i ustawieniami.

1. Otwórz **Biblioteka** → **Nowa instancja**.
2. Wybierz **Wersja Minecraft** i **Loader modów**:
   - **Vanilla:** bez modów.
   - **Fabric** lub **Quilt:** lekkie mody, mody na wydajność.
   - **Forge** lub **NeoForge:** duże mody z nową zawartością.
3. Nadaj jej nazwę i naciśnij **Utwórz instancję**.
4. Naciśnij **Graj**. Pierwsze uruchomienie pobiera grę i właściwą Javę; kolejne są szybkie.

Wskazówka: **Szybkie dołączanie** po lewej pokazuje twoje ostatnie serwery i światy; jedno kliknięcie i już grasz.

## 3. Dodawanie modów, paczek zasobów i shaderów

*Od wersji 0.1.0.*

1. Otwórz swoją instancję → karta **Zawartość** → **Dodaj mody**. Możesz też otworzyć **Odkrywaj** z lewego paska.
2. Wyszukaj albo przeglądaj Modrinth, CurseForge, FTB i inne serwisy. Widać tylko wersje pasujące do twojej instancji.
3. Naciśnij **Zainstaluj**. Wymagane mody (zależności) dodadzą się same.
4. Z powrotem w **Zawartość**:
   - przełącznikiem obok moda włączasz go lub wyłączasz;
   - **Sprawdź aktualizacje** aktualizuje mody (jeden lub wszystkie). **Aktualizuj wszystko** pobiera do czterech naraz.

Niektórzy autorzy z CurseForge nie pozwalają launcherom pobierać swoich plików. Wtedy ResinCraft pokazuje je pod **Pobierz wszystko**: pobierz je w przeglądarce, a ResinCraft sam znajdzie je w folderze Pobrane. Twoja przeglądarka zapisuje pliki gdzie indziej? Dodaj ten folder w **Ustawienia → Pobieranie → Obserwowane foldery**.

## 4. Instalacja modpacka

*Od wersji 0.1.0.*

1. Otwórz **Odkrywaj** → **Modpacki** (albo **Biblioteka** → **Nowa instancja** → **Pobierz modpack**).
2. Otwórz paczkę, żeby zobaczyć jej opis, wersje i **Mody**, które zawiera.
3. Naciśnij **Zainstaluj modpack**. Nowa instancja od razu pojawia się na ekranie **Biblioteka**, z postępem instalacji na karcie; **Graj** działa, gdy instalacja się skończy.
4. Gdy paczka dostanie aktualizację, ResinCraft da ci znać; inną wersję paczki możesz też wybrać na stronie instancji.

## 5. Zmiana wersji Minecrafta lub loadera modów

*Od wersji 1.2.7.*

1. Na stronie instancji kliknij etykietę wersji na górze (na przykład **1.21.4 · Fabric 0.16.10**). Otworzy się **Ustawienia → Wersja**.
2. Wybierz wersję **Minecraft**, **Loader modów** i wersję loadera.
3. Naciśnij **Zmień wersję** i potwierdź. Przy następnym uruchomieniu pobierze się to, czego trzeba.
4. Mody zrobione pod starą wersję mogą nie działać: wejdź w **Zawartość** i naciśnij **Sprawdź aktualizacje**.

## 6. Gdy Minecraft się zawiesi

*Od wersji 1.2.7.*

Gdy gra się zawiesi, otwiera się okno **(nazwa instancji) się zawiesił**. ResinCraft czyta raport awarii i logi i mówi ci, co poszło nie tak, na przykład:

- *Sodium Extra potrzebuje Sodium, który nie jest zainstalowany* → **Napraw** instaluje Sodium.
- *citresewn wysypał grę* → **Napraw** wyłącza ten mod (później włączysz go z powrotem w **Zawartość**).
- *Potrzebna jest Java 21 lub nowsza* → **Napraw** pozwala ResinCraft wybrać właściwą Javę.
- *Minecraftowi zabrakło pamięci* → **Napraw** daje mu więcej pamięci.

Naciśnij **Napraw** przy każdym problemie albo **Napraw wszystko i graj**. Nic się nie zmienia, dopóki nie naciśniesz przycisku. Jeśli ResinCraft nie wie, co się stało, **Otwórz logi** pokaże, co gra napisała na końcu.

## 7. Kopiowanie z innej instancji

*Od wersji 1.2.5.*

1. Otwórz instancję, *do której* chcesz kopiować → naciśnij przycisk **Kopiuj z innej instancji** na górze.
2. Wybierz instancję, *z której* kopiujesz.
3. Zaznacz, co chcesz:
   - **Światy**;
   - **Mody** i **Ustawienia modów** (folder config);
   - **Paczki shaderów** i **Paczki zasobów**;
   - **Ustawienia gry** (grafika, sterowanie, przypisania klawiszy).
4. Naciśnij **Kopiuj**. Światy, których nazwa jest już zajęta, dostają nową nazwę. Mody i paczki, które już masz, zostają bez zmian.

## 8. Hostowanie serwera

*Od wersji 0.1.0.*

1. Otwórz **Servers** → **Create server** (utwórz serwer).
2. Wybierz sposób:
   - **Server software** (oprogramowanie serwera): Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack:** ta sama paczka, w którą grają twoi znajomi.
   - **From an instance** (z instancji): mody, których już używasz.
   - **From a file** (z pliku).
3. Naciśnij **Create**. Okno się zamknie, a pobieranie będzie trwać w prawym dolnym rogu.
4. Gdy pojawi się pytanie, zaakceptuj EULA Minecrafta, a potem naciśnij **Start**.

Na stronie serwera:

- **Console** (konsola): co mówi serwer; polecenia wpisujesz na dole.
- **Players** (gracze): kto jest teraz online, a także whitelista, operatorzy i bany.
- **Mods** lub **Plugins:** dodawanie, aktualizowanie i wyłączanie. **Zaktualizuj wszystko** pobiera do czterech naraz; gdy serwer działa, aktualizacje wchodzą przy następnym restarcie.
- **Properties** (właściwości): ustawienia serwera (poziom trudności, whitelista wł./wył. i tak dalej).
- **Settings** (ustawienia):
  - pamięć, Java i **Java flags** (flagi ResinCraft albo flagi samego modpacka);
  - jak łączą się znajomi;
  - ikona serwera.
- **Uruchom ponownie** i menu **⋯** (kopiowanie adresu, otwieranie folderu, kopia zapasowa teraz, usuwanie).

- **Settings → Anti-xray** (ustawienia → ochrona przed x-rayem; Paper, Purpur, Folia, Leaf): ukrywa rudy przed modami i paczkami tekstur typu x-ray. **Hide ores** (ukryj rudy) mało obciąża serwer; **Fake ores** (fałszywe rudy) działa mocniej. Zmiana działa od następnego startu.
- **Settings → Anti-cheat** (ustawienia → ochrona przed oszustwami; serwery z pluginami): jeden przełącznik instaluje Grim.

Mody, które działają tylko po stronie klienta gry (shadery, minimapy…), zostają na serwerze, dopóki ten startuje bez problemu. Jeśli któryś go wysypie, ResinCraft wyłącza ten mod i uruchamia serwer ponownie; w karcie **Mods** możesz go włączyć z powrotem.

Gdy modpack nie ma oficjalnej paczki serwerowej, ResinCraft buduje serwer z listy modów samej paczki: mówi o tym okno po naciśnięciu **Create**, a serwer dostaje plakietkę **Built by ResinCraft** (zbudowane przez ResinCraft). Niektóre pliki paczki można pobrać tylko ze strony ich autora. Są wypisane na górze karty **Console**, z przyciskiem **Get file** (pobierz plik). Pobierz je w przeglądarce, a ResinCraft umieści je na miejscu przed następnym startem; możesz też nacisnąć **Look again** (szukaj ponownie).

## 9. Jak wpuścić znajomych na serwer

*Od wersji 0.2.0.*

W **Settings** serwera → **How friends connect** (jak łączą się znajomi):

- **Automatic** (zalecane i domyślne dla nowych serwerów): najpierw twój router, potem **ResinCraft Relay**, potem playit.gg.
- **ResinCraft Relay:** znajomi dołączają przez `yourname.rscraft.resincore.tech`. Wymaga zalogowanego konta Microsoft.
- **playit.gg:** darmowy adres, który się nie zmienia. Przy pierwszym starcie poprosi cię o połączenie konta playit.gg (konto gościa wystarczy).
- **Port przekierowany samodzielnie:** jeśli sam ustawiłeś przekierowanie na routerze.

Karta **Console** pokazuje adres, który podajesz znajomym. Znajomi z Bedrock też mogą dołączyć do serwera Java: włącz **Bedrock players can join** (gracze Bedrock mogą dołączyć).

**Własna domena:** z ResinCraft Relay włącz **Podłącz własną domenę do ResinCraft Relay** i wpisz domenę. Dodaj pokazane rekordy DNS, w tym rekord TXT, który potwierdza, że domena jest twoja, a potem naciśnij **Zweryfikuj**. Plakietka zmieni się z **Oczekuje na weryfikację** na **Zweryfikowano**. Gdy automatyczny DNS w Cloudflare jest włączony, ResinCraft sam doda rekordy.

## 10. Prośby o whitelistę (strona WWW i bot Discord)

*Od wersji 1.2.7.*

Pozwól graczom prosić o dołączenie, a sam ich akceptuj lub odrzucaj.

1. W **Properties** ustaw **white-list** na **true** (inaczej i tak każdy może dołączyć).
2. Otwórz kartę **Players** serwera i włącz **Whitelist requests** (prośby o whitelistę).
3. Wybierz, jak gracze proszą:
   - **Web page** (strona WWW):
     - Działa, gdy serwer jest udostępniany przez ResinCraft Relay.
     - Naciśnij **Copy link** (kopiuj link) i wyślij graczom link. To własny adres twojego serwera albo twoja zweryfikowana domena.
     - Strona działa, dopóki serwer jest online.
   - **Your Discord bot** (twój bot Discord):
     1. Otwórz [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token** i skopiuj token.
     2. Wklej go w ResinCraft i naciśnij **Save** (zapisz). Jest bezpiecznie przechowywany na twoim komputerze i już nigdy nie zostanie pokazany.
     3. Gdy pojawi się **Online as …**, naciśnij **Add the bot to your Discord** (dodaj bota do swojego Discorda) i wybierz swój serwer Discord.
     4. Gracze wpisują `/whitelist TheirName` na twoim Discordzie.
4. Prośby pojawiają się w karcie **Players**:
   - **Accept** (akceptuj): dodaje gracza do whitelisty (bot daje mu znać w wiadomości prywatnej).
   - **Reject** (odrzuć): odrzuca prośbę.
   - **Reject and block** (odrzuć i zablokuj): odrzuca ją i sprawia, że ta osoba nie może poprosić ponownie. Zablokowani gracze są na liście **Blocked** (zablokowani), z przyciskiem **Unblock** (odblokuj) przy każdym.
   - **Z listy serwerów:** gracze z ResinCraft mogą poprosić ze strony twojego serwera w Odkrywaj → Serwery (sekcja 17).

Prośby zbanowanych graczy są odrzucane automatycznie. Bot Discord działa, dopóki ResinCraft jest otwarty; włącz zasobnik systemowy (sekcja 13), żeby działał także po zamknięciu okna.

## 11. Bedrock Edition

*Od wersji 0.3.1.*

Na Windowsie **Bedrock** po lewej instaluje, uruchamia i odinstalowuje Minecraft for Windows i Minecraft Preview. Przed odinstalowaniem robi kopię zapasową twoich światów.

## 12. Przenosiny z innego launchera

*Od wersji 1.0.5.*

**Biblioteka** → **Przenieś z innego launchera** sprowadza instancje z:

- Prism Launcher i MultiMC
- aplikacji Modrinth i aplikacji CurseForge
- ATLauncher i GDLauncher
- oficjalnego Minecraft Launchera

Ich mody, światy i ustawienia przechodzą razem z nimi. Oryginały zostają tam, gdzie są.

## 13. Aktualizacje i zasobnik systemowy

*Od wersji 0.3.0 (zasobnik systemowy od 1.2.7).*

- **Aktualizacje:**
  - Gdy nowa wersja ResinCraft jest gotowa, na pasku tytułu pojawia się przycisk **Aktualizuj**.
  - Jeśli działa gra albo serwer, wybierasz **Zamknij i zaktualizuj** (serwery najpierw zapisują swoje światy) albo **Aktualizuj, gdy się zatrzymają**.
- **Aktualizuj po cichu** (Ustawienia → Aktualizacje): instaluje nowe wersje w tle, bez pytania, gdy nic nie jest uruchomione.
- **Działaj w zasobniku systemowym** (Ustawienia → Wygląd):
  - Zamknięcie okna nie wyłącza ResinCraft: serwery, bot Discord i pobieranie działają dalej.
  - Kliknij ikonę w zasobniku, żeby znów otworzyć okno; prawy przycisk → **Quit ResinCraft** (zamknij ResinCraft), żeby naprawdę wyjść.

## 14. Motywy i własny CSS

*Od wersji 1.3.0.*

1. Otwórz **Ustawienia → Wygląd → Motyw** i wybierz kartę: **Systemowy**, Ciemny, Jasny, Obsydian, Łupek, Wiśnia, Nether lub Kres.
2. **Własny CSS:** naciśnij **Wstaw obecne kolory**, zmień dowolny kolor, a potem **Zastosuj**. Działa na każdym motywie; **Wyczyść** go usuwa.
3. Otwórz **Jak dostosować** pod polem: znajdziesz tam kolory i kształty, które możesz zmienić, części aplikacji, które możesz ostylować, i przykłady (obraz w tle, bardziej zaokrąglone rogi, większy tekst…). Kliknij jeden, żeby dodać go do swojego CSS.

Psst: spróbuj kodu Konami (↑ ↑ ↓ ↓ ← → ← → B A) albo kliknij logo ResinCraft siedem razy.

## 15. Zdalne sterowanie (konsola webowa)

*Od wersji 1.3.0.*

Steruj swoimi serwerami z dowolnej przeglądarki albo z telefonu.

1. Zaloguj się kontem Microsoft w **Konta**.
2. Włącz **Ustawienia → Zdalne sterowanie → Zezwól na zdalne sterowanie**. Gdy pojawi się **Połączono jako …**, wszystko gotowe.
3. Włącz **Działaj w zasobniku systemowym** (sekcja 13), żeby działało też po zamknięciu okna.
4. Otwórz `https://console.rscraft.resincore.tech` (albo naciśnij **Otwórz konsolę webową**) i zaloguj się **tym samym kontem Minecraft**.

Z konsoli możesz robić wszystko, co menedżer serwerów: uruchamiać, zatrzymywać i restartować serwery, podglądać konsolę i wpisywać polecenia, zarządzać graczami i prośbami o whitelistę, dodawać i aktualizować pluginy i mody, zmieniać właściwości i ustawienia, zmieniać wersję modpacka, instalować ponownie, robić kopie zapasowe i tworzyć nowe serwery. Każda zmiana pojawia się w ResinCraft jako powiadomienie.

Menedżer plików, ustawienia Javy i pluginy z dowolnego linku do pobrania mogą uruchamiać programy na twoim PC, dlatego wymagają drugiego przełącznika: **Zezwól na pełną kontrolę**. Włącz go tylko wtedy, gdy twoje konto Minecraft (Microsoft) ma weryfikację dwuetapową. Bez niego konsola nigdy nie ma dostępu do niczego innego na twoim PC.

## 16. Ponowna instalacja i naprawa

*Od wersji 1.3.0.*

- **Instancja się nie uruchamia, bo brakuje plików albo są uszkodzone:**
  - Kliknij ją prawym przyciskiem na ekranie **Biblioteka** → **Zainstaluj ponownie** → **Pliki gry** albo otwórz jej **Ustawienia → Naprawa**.
  - Minecraft, loader modów, biblioteki i zasoby pobierają się ponownie. Światy, mody i ustawienia zostają.
- **Pliki modpacka zostały zmienione lub usunięte:** **Zainstaluj ponownie** → **Pliki modpacka** przywraca mody, konfiguracje i skrypty paczki. Zmienione przez ciebie pliki są najpierw kopiowane; światy i twoje własne mody zostają.
- **Serwer:** jego menu **⋯** → **Zainstaluj ponownie** → **Oprogramowanie serwera** (albo **Pliki modpacka** dla serwera z modpacka). Świat, ustawienia i listy graczy zostają.
- **Sam ResinCraft:** **Ustawienia → Aktualizacje → Zainstaluj ponownie ResinCraft** instaluje aplikację ponownie na sobie.

## 17. Szukanie serwerów i dołączanie

*Od wersji 1.4.0.*

1. Otwórz **Odkrywaj** → **Serwery**. Lista pokazuje serwery hostowane z ResinCraft i serwery dodane przez graczy.
2. Wyszukaj albo filtruj według kategorii (SMP, PvP, Minigry, Skyblock…), regionu, języka, modpacka i tego, czy mogą dołączyć gracze Bedrock lub profile offline.
3. Kliknij ikonę lub nazwę serwera, żeby otworzyć jego stronę: pełny opis, wszystkie szczegóły, jego paczkę klienta i linki.
4. Naciśnij **Dołącz**. ResinCraft wybiera instancję, która pasuje do serwera, albo pyta, której użyć. Serwer z własnymi modami ma **Zainstaluj i dołącz**: tworzy instancję z dokładnie tymi modami i dołącza.
5. Serwer z whitelistą? Naciśnij **Poproś o dołączenie**. Przycisk pokazuje **Waiting for the host** (czekam na gospodarza), a potem **On the whitelist** (na whiteliście; naciśnij **Dołącz**) albo **Ask again** (poproś ponownie). Gdy gospodarz odpowie, ResinCraft daje ci znać i miga paskiem zadań.

Polub serwery, które ci się podobają, i zgłoś te, które łamią zasady.

## 18. Umieść swój serwer na liście i udostępnij jego mody

*Od wersji 1.4.0.*

1. W **Settings** hostowanego serwera włącz **Lista serwerów** (domyślnie jest wyłączona).
2. Dodaj opis, tagi, region i język. Obsługa Bedrock i profili offline oraz modpack uzupełniają się same.
3. **Paczka klienta:** opublikuj to, czego potrzebują gracze, żeby mogli użyć **Zainstaluj i dołącz**:
   - modpack serwera, instalowany z jego własnego serwisu;
   - każdy inny mod, z Modrinth lub CurseForge;
   - twoje własne pliki, wysyłane prosto z twojego PC, gdy działa ResinCraft;
   - paczki zasobów i shadery, które zdecydujesz się dołączyć.
   Serwery z pluginami i vanilla też mogą ją mieć: wybierz instancję z wersją Minecrafta serwera.

Serwer działa gdzie indziej? Dodaj go przez **Zgłoś serwer** w Odkrywaj → Serwery.

## 19. Twoje inne komputery i znajomi, którzy pomagają

*Od wersji 1.4.1.*

**Twoje inne komputery:** włącz zdalne sterowanie na każdym komputerze (sekcja 15) i zaloguj się tym samym kontem Minecraft. Potem w **Servers** otwórz menu komputera na górze i wybierz komputer. Stąd możesz uruchamiać, zatrzymywać i zarządzać jego serwerami, a także użyć tam **Create server** (utwórz serwer) lub **Importuj serwer**. Żeby zaimportować z folderu na tamtym komputerze, wpisz folder; wymaga to włączonego **Zezwól na pełną kontrolę** na tamtym komputerze.

**Znajomi, którzy pomagają prowadzić twoje serwery:**

1. Otwórz **Ustawienia → Zdalne sterowanie → People who can use your servers** (osoby, które mogą używać twoich serwerów).
2. Wpisz ich nick w Minecrafcie i naciśnij **Add** (dodaj).
3. Wybierz, co mogą robić:
   - **Viewer** (podgląd): widzi konsolę, graczy i ustawienia.
   - **Operator** (operator): do tego uruchamia, zatrzymuje i restartuje, wysyła polecenia oraz obsługuje graczy i prośby o whitelistę.
   - **Manager** (menedżer): do tego zmienia ustawienia, pluginy i mody oraz modpack.
4. Zostaw **All servers** (wszystkie serwery) albo wybierz serwery w **Which servers?** (które serwery?).

Otwierają twoje serwery w swoim ResinCraft (menu komputera w **Servers**) albo w konsoli webowej, dopóki zdalne sterowanie jest włączone, a ResinCraft działa na twoim komputerze. Pliki, ustawienia Javy oraz tworzenie i usuwanie serwerów zostają tylko twoje.

## 20. Kopie zapasowe światów

*Od wersji 1.4.0.*

- **Automatyczne:** gdy działa gra lub serwer, kopia jego światów powstaje co 30 minut, a zostają 3 najnowsze dla każdego świata. Zmienisz to lub wyłączysz w **Ustawienia → Automatyczna kopia**.
- **Przywracanie:** w karcie **Światy** instancji albo w **Settings → Backups** (ustawienia → kopie zapasowe) hostowanego serwera naciśnij **Przywróć** przy kopii, twojej lub automatycznej. Zastępowany świat trafia do Kosza, więc przywrócenie można cofnąć. Świat jednoosobowy można też odzyskać jako kopię.

## 21. Miejsce na dysku i przenosiny na inny komputer

*Od wersji 1.1.0 (Przenieś na inny komputer od 1.1.1).*

- **Inny dysk:** **Ustawienia → Pamięć → Zmień…**, wybierz dysk lub folder, a potem:
  - **Przenieś instancje:** przenosi tam twoje instancje, serwery i wspólne pliki Minecrafta.
  - **Użyj bez przenoszenia:** nowe rzeczy trafiają tam; obecne instancje zostają i wracają, gdy przełączysz się z powrotem.
- **Nowy komputer:** **Ustawienia → Przenieś na inny komputer** zapisuje wszystko w jednym pliku `.resincraft`. Otwórz go w ResinCraft na nowym komputerze.

## 22. Rozwiązywanie problemów

- **Gra się nie uruchamia:** zajrzyj do okna **(nazwa instancji) się zawiesił** (sekcja 6) albo do karty **Logi** instancji. Brakuje plików albo są uszkodzone? Zainstaluj je ponownie (sekcja 16).
- **Mod nie pojawia się w Odkrywaj:** może nie istnieć dla twojej wersji Minecrafta lub loadera. Spróbuj innej wersji albo zmień wersję instancji (sekcja 5).
- **Znajomi nie mogą dołączyć:** sprawdź adres w karcie **Console** serwera i czy serwer pokazuje **Running** (działa). Przy ResinCraft Relay upewnij się, że jesteś zalogowany kontem Microsoft.
- **Serwer wysypuje się przy starcie:** ResinCraft sam wyłącza mody tylko dla klienta i uruchamia go ponownie. Poszukaj w **Console** linijki, która mówi, który mod wyłączył. Jeśli mówi, że brakuje plików, pobierz je z listy na górze karty **Console**.
- **Dalej nie działa?** Zgłoś problem na [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) i dołącz log z karty **Logi**.
