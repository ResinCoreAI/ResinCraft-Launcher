# ResinCraft Launcher: poradnik

Jak zrobić w ResinCraft najczęstsze rzeczy. Nazwy **pogrubione** to to, co zobaczysz na ekranie.

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
14. [Rozwiązywanie problemów](#14-rozwiązywanie-problemów)

## 1. Instalacja i logowanie

1. Pobierz ResinCraft dla swojego systemu ze [strony Releases](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** `…_amd64_windows.exe`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` lub `.tar.gz`
2. Zainstaluj i uruchom. Windows może ostrzec, że wydawca jest niezweryfikowany; na macOS za pierwszym razem kliknij aplikację prawym przyciskiem i wybierz **Otwórz**.
3. Otwórz **Konta** → **Zaloguj przez Microsoft** i zaloguj się na stronie Microsoftu. Twoje hasło nigdy nie przechodzi przez ResinCraft.
   - Nie masz konta Microsoft? Dodaj **Profil offline** do gry jednoosobowej i w sieci LAN.

## 2. Twoja pierwsza instancja

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

1. Otwórz swoją instancję → karta **Zawartość** → **Dodaj mody**. Możesz też otworzyć **Odkrywaj** z lewego paska.
2. Wyszukaj albo przeglądaj Modrinth, CurseForge, FTB i inne serwisy. Widać tylko wersje pasujące do twojej instancji.
3. Naciśnij **Zainstaluj**. Wymagane mody (zależności) dodadzą się same.
4. Z powrotem w **Zawartość**:
   - przełącznikiem obok moda włączasz go lub wyłączasz;
   - **Sprawdź aktualizacje** aktualizuje mody (jeden lub wszystkie).

Niektórzy autorzy z CurseForge nie pozwalają launcherom pobierać swoich plików. Wtedy ResinCraft pokazuje je pod **Pobierz wszystko**: pobierz je w przeglądarce, a ResinCraft sam znajdzie je w folderze Pobrane. Twoja przeglądarka zapisuje pliki gdzie indziej? Dodaj ten folder w **Ustawienia → Pobieranie → Obserwowane foldery**.

## 4. Instalacja modpacka

1. Otwórz **Odkrywaj** → **Modpacki** (albo **Biblioteka** → **Nowa instancja** → **Pobierz modpack**).
2. Otwórz paczkę, żeby zobaczyć jej opis, wersje i **Mody**, które zawiera.
3. Naciśnij **Zainstaluj modpack**. ResinCraft tworzy nową instancję ze wszystkim w środku.
4. Gdy paczka dostanie aktualizację, ResinCraft da ci znać; inną wersję paczki możesz też wybrać na stronie instancji.

## 5. Zmiana wersji Minecrafta lub loadera modów

1. Na stronie instancji kliknij etykietę wersji na górze (na przykład **1.21.4 · Fabric 0.16.10**). Otworzy się **Ustawienia → Wersja**.
2. Wybierz wersję **Minecraft**, **Loader modów** i wersję loadera.
3. Naciśnij **Zmień wersję** i potwierdź. Przy następnym uruchomieniu pobierze się to, czego trzeba.
4. Mody zrobione pod starą wersję mogą nie działać: wejdź w **Zawartość** i naciśnij **Sprawdź aktualizacje**.

## 6. Gdy Minecraft się zawiesi

Gdy gra się zawiesi, otwiera się okno **(nazwa instancji) się zawiesił**. ResinCraft czyta raport awarii i logi i mówi ci, co poszło nie tak, na przykład:

- *Sodium Extra potrzebuje Sodium, który nie jest zainstalowany* → **Napraw** instaluje Sodium.
- *citresewn wysypał grę* → **Napraw** wyłącza ten mod (później włączysz go z powrotem w **Zawartość**).
- *Potrzebna jest Java 21 lub nowsza* → **Napraw** pozwala ResinCraft wybrać właściwą Javę.
- *Minecraftowi zabrakło pamięci* → **Napraw** daje mu więcej pamięci.

Naciśnij **Napraw** przy każdym problemie albo **Napraw wszystko i graj**. Nic się nie zmienia, dopóki nie naciśniesz przycisku. Jeśli ResinCraft nie wie, co się stało, **Otwórz logi** pokaże, co gra napisała na końcu.

## 7. Kopiowanie z innej instancji

1. Otwórz instancję, *do której* chcesz kopiować → naciśnij przycisk **Kopiuj z innej instancji** na górze.
2. Wybierz instancję, *z której* kopiujesz.
3. Zaznacz, co chcesz:
   - **Światy**;
   - **Mody** i **Ustawienia modów** (folder config);
   - **Paczki shaderów** i **Paczki zasobów**;
   - **Ustawienia gry** (grafika, sterowanie, przypisania klawiszy).
4. Naciśnij **Kopiuj**. Światy, których nazwa jest już zajęta, dostają nową nazwę. Mody i paczki, które już masz, zostają bez zmian.

## 8. Hostowanie serwera

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
- **Mods** lub **Plugins:** dodawanie, aktualizowanie i wyłączanie.
- **Properties** (właściwości): ustawienia serwera (poziom trudności, whitelista wł./wył. i tak dalej).
- **Settings** (ustawienia):
  - pamięć, Java i **Java flags** (flagi ResinCraft albo flagi samego modpacka);
  - jak łączą się znajomi;
  - ikona serwera.
- **Uruchom ponownie** i menu **⋯** (kopiowanie adresu, otwieranie folderu, kopia zapasowa teraz, usuwanie).

Mody, które działają tylko po stronie klienta gry (shadery, minimapy…), są na serwerach wyłączane automatycznie. Zobaczysz je w karcie **Mods** i w razie potrzeby włączysz z powrotem.

## 9. Jak wpuścić znajomych na serwer

W **Settings** serwera → **How friends connect** (jak łączą się znajomi):

- **Automatic** (zalecane): najpierw próbuje **ResinCraft Relay**, potem playit.gg, potem bore.
- **ResinCraft Relay:** znajomi dołączają przez `yourname.rscraft.resincore.tech`. Wymaga zalogowanego konta Microsoft.
- **playit.gg:** darmowy adres, który się nie zmienia. Przy pierwszym starcie poprosi cię o połączenie konta playit.gg (konto gościa wystarczy).
- **Port przekierowany samodzielnie:** jeśli sam ustawiłeś przekierowanie na routerze.

Karta **Console** pokazuje adres, który podajesz znajomym. Znajomi z Bedrock też mogą dołączyć do serwera Java: włącz **Bedrock players can join** (gracze Bedrock mogą dołączyć).

## 10. Prośby o whitelistę (strona WWW i bot Discord)

Pozwól graczom prosić o dołączenie, a sam ich akceptuj lub odrzucaj.

1. W **Properties** ustaw **white-list** na **true** (inaczej i tak każdy może dołączyć).
2. Otwórz kartę **Players** serwera i włącz **Whitelist requests** (prośby o whitelistę).
3. Wybierz, jak gracze proszą:
   - **Web page** (strona WWW):
     - Działa, gdy serwer jest udostępniany przez ResinCraft Relay.
     - Naciśnij **Copy link** (kopiuj link) i wyślij graczom link: `https://rscraft.resincore.tech/join/<name>`.
     - Strona działa, dopóki serwer jest online.
   - **Your Discord bot** (twój bot Discord):
     1. Otwórz [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token** i skopiuj token.
     2. Wklej go w ResinCraft i naciśnij **Save** (zapisz). Jest bezpiecznie przechowywany na twoim komputerze i już nigdy nie zostanie pokazany.
     3. Gdy pojawi się **Online as …**, naciśnij **Add the bot to your Discord** (dodaj bota do swojego Discorda) i wybierz swój serwer Discord.
     4. Gracze wpisują `/whitelist TheirName` na twoim Discordzie.
4. Prośby pojawiają się w karcie **Players**:
   - **Accept** (akceptuj): dodaje gracza do whitelisty (bot daje mu znać w wiadomości prywatnej).
   - **Reject** (odrzuć): odrzuca prośbę.
   - **Reject and block** (odrzuć i zablokuj): odrzuca ją i sprawia, że ta osoba nie może poprosić ponownie. **Unblock all** (odblokuj wszystkich) cofa blokady z Discorda.

Prośby zbanowanych graczy są odrzucane automatycznie. Bot Discord działa, dopóki ResinCraft jest otwarty; włącz zasobnik systemowy (sekcja 13), żeby działał także po zamknięciu okna.

## 11. Bedrock Edition

Na Windowsie **Bedrock** po lewej instaluje, uruchamia i odinstalowuje Minecraft for Windows i Minecraft Preview. Przed odinstalowaniem robi kopię zapasową twoich światów.

## 12. Przenosiny z innego launchera

**Biblioteka** → **Przenieś z innego launchera** sprowadza instancje z:

- Prism Launcher i MultiMC
- aplikacji Modrinth i aplikacji CurseForge
- ATLauncher i GDLauncher
- oficjalnego Minecraft Launchera

Ich mody, światy i ustawienia przechodzą razem z nimi. Oryginały zostają tam, gdzie są.

## 13. Aktualizacje i zasobnik systemowy

- **Aktualizacje:**
  - Gdy nowa wersja ResinCraft jest gotowa, na pasku tytułu pojawia się przycisk **Aktualizuj**.
  - Jeśli działa gra albo serwer, wybierasz **Zamknij i zaktualizuj** (serwery najpierw zapisują swoje światy) albo **Aktualizuj, gdy się zatrzymają**.
- **Aktualizuj po cichu** (Ustawienia → Aktualizacje): instaluje nowe wersje w tle, bez pytania, gdy nic nie jest uruchomione.
- **Działaj w zasobniku systemowym** (Ustawienia → Wygląd):
  - Zamknięcie okna nie wyłącza ResinCraft: serwery, bot Discord i pobieranie działają dalej.
  - Kliknij ikonę w zasobniku, żeby znów otworzyć okno; prawy przycisk → **Quit ResinCraft** (zamknij ResinCraft), żeby naprawdę wyjść.

## 14. Rozwiązywanie problemów

- **Gra się nie uruchamia:** zajrzyj do okna **(nazwa instancji) się zawiesił** (sekcja 6) albo do karty **Logi** instancji.
- **Mod nie pojawia się w Odkrywaj:** może nie istnieć dla twojej wersji Minecrafta lub loadera. Spróbuj innej wersji albo zmień wersję instancji (sekcja 5).
- **Znajomi nie mogą dołączyć:** sprawdź adres w karcie **Console** serwera i czy serwer pokazuje **Running** (działa). Przy ResinCraft Relay upewnij się, że jesteś zalogowany kontem Microsoft.
- **Serwer wysypuje się przy starcie:** ResinCraft sam wyłącza mody tylko dla klienta i uruchamia go ponownie. Poszukaj w **Console** linijki, która mówi, który mod wyłączył.
- **Dalej nie działa?** Zgłoś problem na [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) i dołącz log z karty **Logi**.
