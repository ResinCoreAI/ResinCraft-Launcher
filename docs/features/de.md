# ResinCraft Launcher: Funktionen

Ein moderner, freundlicher Minecraft-Launcher von **Puirkle**, für Windows, macOS und Linux. Keine Telemetrie und kein ResinCraft-Konto.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Mit **(1.2.7)** markierte Punkte sind neu in Version 1.2.7.

## Spielen

- **Instanzen:** getrennte Installationen pro Version oder Modpack, jede mit eigenen Mods, Welten, Java, Arbeitsspeicher und JVM-Argumenten.
- **Jede Minecraft-Version:** alle Releases und Snapshots, bis zurück zu den Classic-Alphas.
- **Mod-Loader:** Fabric, Quilt, Forge (bis zurück zu Minecraft 1.2.5), NeoForge und LiteLoader, mit Auswahl der Loader-Version. Auch OptiFine und Jar-Mods.
- **Version und Loader ändern (1.2.7):** Minecraft-Version oder Mod-Loader einer Instanz im Tab Einstellungen wechseln, wie in Prism. Der Versions-Chip auf der Instanzseite führt direkt dorthin.
- **Spielen:** ein Klick. Mit **Diesmal spielen als** ein anderes Konto nehmen, oder mit **Zusätzlich spielen als** dieselbe Instanz zweimal mit zwei Konten starten.
- **Schnellbeitritt:** deine letzten Server und Einzelspielerwelten; ein Klick, und du bist direkt drin.
- **Welten:** eine Welt direkt spielen, umbenennen, sichern oder eine Welt aus einer ZIP-Datei importieren.
- **Absturzhilfe (1.2.7):** Stürzt Minecraft ab, liest ResinCraft den Absturzbericht und die Logs. Es erklärt dir in einfachen Worten, was schiefgelaufen ist, und bietet eine Lösung mit einem Klick an. Lösungen: einen fehlenden Mod installieren, einen abgestürzten Mod ausschalten, das richtige Java wählen, dem Spiel mehr Arbeitsspeicher geben.
- **Java:** findet installierte Java-Versionen, wählt für jede Minecraft-Version die passende und lädt Eclipse Temurin herunter, wenn keine passt.
- **RAM-Anzeige:** wie viel Arbeitsspeicher der Launcher, jedes Spiel und jeder Server belegen (auf Wunsch auch auf der Startseite).
- **Discord Rich Presence:** Packname, Version, Spielzeit, Einzelspieler oder Server. Buttons für Freunde: **View modpack** (Modpack ansehen) und **Get ResinCraft** (ResinCraft holen).

## Instanzen

- **Ordnen:** Gruppen, eigene Reihenfolge (per Ziehen anordnen), Symbole, Banner und Farben.
- **Tabs:** Inhalte, Welten, Server, Screenshots, Notizen, Dateien (eingebauter Dateimanager und Editor), Logs und Einstellungen.
- **Aus anderer Instanz kopieren:** Welten, Mods und ihre Einstellungen (config-Ordner), Shaderpacks, Ressourcenpakete und Spieleinstellungen.
- **Duplizieren, exportieren** (Modrinth `.mrpack` oder CurseForge `.zip`) und **zu deinem eigenen Modpack machen**.
- **Befehle und Umgebung:** Befehle, die vor dem Start und nach dem Schließen des Spiels laufen, sowie Umgebungsvariablen.
- **Sofortige Schalter (1.2.7):** Einen Mod ein- oder auszuschalten geht sofort; das Umbenennen passiert im Hintergrund.

## Inhalte und Entdecken

- **Quellen:** Mods, Modpacks, Ressourcenpakete und Shader von Modrinth, CurseForge, FTB, ATLauncher, Technic und GitHub-Releases, gefiltert nach Version und Loader der Instanz.
- **Modpack-Seiten** mit Beschreibung, Galerie, Versionen, Changelogs und **den Mods, die im Pack stecken**.
- **Clever installieren:** jede beliebige Version wählen, mehrere auf einmal installieren, benötigte Abhängigkeiten kommen automatisch dazu, und derselbe Mod von zwei Seiten wird erkannt.
- **Updates:** Updates suchen und einspielen (einzeln oder alle); Modpack-Updates und Versionswechsel.
- **Manuelle Downloads:** CurseForge-Dateien, deren Autoren Launcher blockieren, werden aus deinem Downloads-Ordner oder anderen **Überwachte Ordner** übernommen (Einstellungen → Downloads).
- **Listen:** die Mods, Ressourcenpakete oder Shader als Liste exportieren und eine Liste in eine andere Instanz installieren.

## Server hosten

- **Servertypen:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (Mods und Plugins zusammen) und Bedrock Dedicated Server.
- **Aus einem Modpack oder einer Instanz:** einen Server aus Modrinth, CurseForge (offizielles Server-Pack, falls es eins gibt) oder FTB erstellen, oder aus einer deiner Instanzen.
- **Reine Client-Mods bleiben aus:** Mods, die nur im Spiel-Client laufen, werden bei jedem Serverstart gefunden und automatisch ausgeschaltet. Ein Mod, den ein anderer Mod braucht, bleibt an. Bringt trotzdem einer den Server zum Absturz, wird er ausgeschaltet und der Server startet von selbst neu.
- **Verwalten:** Konsole, Properties-Editor, Plugin- und Mod-Verwaltung mit Updates, Whitelist, Ops und Bans, Server-Icon, Backups.
- **Neu starten und Mehr-Menü (1.2.7):** Neu starten neben Stop; ein Menü mit Adresse kopieren, Ordner öffnen, sichern, Einstellungen und Löschen.
- **Online now** (gerade online): die Spieler auf deinem Server, alle paar Sekunden live geprüft **(1.2.7)**.
- **Java flags (1.2.7)** (Java-Flags): die abgestimmten Flags von ResinCraft oder die eigenen des Modpacks (`user_jvm_args.txt`, Startskripte).
- **Mit Freunden teilen:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - dein Router (UPnP)
  - deine eigene Domain, mit automatischem Cloudflare-DNS
- **Bedrock players can join** (Bedrock-Spieler können beitreten): Bedrock-Spieler kommen auch auf Java-Server (Geyser + Floodgate).
- **Whitelist requests (1.2.7)** (Whitelist-Anfragen): Spieler fragen, ob sie beitreten dürfen, und du nimmst sie im Tab Players an oder lehnst sie ab.
  - **Web page** (Webseite): `https://rscraft.resincore.tech/join/<name>` (mit ResinCraft Relay).
  - **Your Discord bot** (dein eigener Discord-Bot): Spieler tippen `/whitelist`.
  - **Deine Antwort:** Accept (annehmen) setzt sie auf die Whitelist; der Bot sagt es ihnen per Direktnachricht.
  - **Schutz:** Gebannte Spieler werden abgelehnt; **Reject and block** (ablehnen und blockieren) verhindert, dass jemand erneut fragt.

## Bedrock Edition (Windows)

- Minecraft für Windows und Minecraft Preview installieren, starten und deinstallieren (die Welten werden vorher gesichert).

## Konten

- **Microsoft-Anmeldung**, mit verschlüsselt gespeicherten Tokens. **Offline-Profile** für Einzelspieler und LAN.
- **Skins, Umhänge und Namensänderung.**

## Von einem anderen Launcher umziehen

- Instanzen aus Prism Launcher, MultiMC, der Modrinth App, der CurseForge-App, ATLauncher, GDLauncher und dem offiziellen Minecraft Launcher importieren, samt Mods, Welten, Packs und Einstellungen. Auch Server lassen sich importieren.

## Die App

- **Windows, macOS** (Apple Silicon und Intel) **und Linux** (x64 und ARM64: AppImage, `.deb`, `.tar.gz`).
- **Updates:** werden automatisch gesucht.
  - **(1.2.7)** Ein **Update**-Button in der Titelleiste.
  - **(1.2.7)** **Schließen und aktualisieren**, wenn Spiele oder Server laufen.
  - **(1.2.7)** **Still aktualisieren** im Hintergrund.
- **Im Infobereich weiterlaufen (1.2.7):** Wenn du das Fenster schließt, laufen Server, der Discord-Bot und Downloads weiter.
- **16 Sprachen:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Speicher** verwalten (Daten auf ein anderes Laufwerk verschieben), **Sichern und Wiederherstellen** der Einstellungen, **helles und dunkles** Design.
