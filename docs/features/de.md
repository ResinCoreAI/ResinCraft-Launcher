# ResinCraft Launcher: Funktionen

Ein moderner, freundlicher Minecraft-Launcher von **Puirkle**, für Windows, macOS und Linux. Keine Telemetrie und kein ResinCraft-Konto.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Mit **(neu)** markierte Punkte sind neu in der neuesten Version.

## Spielen

- **Instanzen:** getrennte Installationen pro Version oder Modpack, jede mit eigenen Mods, Welten, Java, Arbeitsspeicher und JVM-Argumenten.
- **Jede Minecraft-Version:** alle Releases und Snapshots, bis zurück zu den Classic-Alphas.
- **Mod-Loader:** Fabric, Quilt, Forge (bis zurück zu Minecraft 1.2.5), NeoForge und LiteLoader, mit Auswahl der Loader-Version. Auch OptiFine und Jar-Mods.
- **Version und Loader ändern:** Minecraft-Version oder Mod-Loader einer Instanz im Tab Einstellungen wechseln, wie in Prism. Der Versions-Chip auf der Instanzseite führt direkt dorthin.
- **Spielen:** ein Klick. Mit **Diesmal spielen als** ein anderes Konto nehmen, oder mit **Zusätzlich spielen als** dieselbe Instanz zweimal mit zwei Konten starten.
- **Schnellbeitritt:** deine letzten Server und Einzelspielerwelten; ein Klick, und du bist direkt drin.
- **Welten:** eine Welt direkt spielen, umbenennen, sichern oder eine Welt aus einer ZIP-Datei importieren.
- **Absturzhilfe:** Stürzt Minecraft ab, liest ResinCraft den Absturzbericht und die Logs. Es erklärt dir in einfachen Worten, was schiefgelaufen ist, und bietet eine Lösung mit einem Klick an. Lösungen: einen fehlenden Mod installieren, einen abgestürzten Mod ausschalten, das richtige Java wählen, dem Spiel mehr Arbeitsspeicher geben.
- **Neu installieren (neu):** die Spieldateien einer Instanz erneut herunterladen oder die Dateien eines Modpacks wieder so ablegen, wie das Pack sie hat. Über das Menü der Instanz oder **Einstellungen → Reparieren**. Welten bleiben.
- **Abgestimmte Java-Flags:** Spiele starten mit den G1GC-Flags von ResinCraft für flüssigere Bilder (Einstellungen → Java, oder pro Instanz).
- **Java:** findet installierte Java-Versionen, wählt für jede Minecraft-Version die passende und lädt Eclipse Temurin herunter, wenn keine passt.
- **RAM-Anzeige:** wie viel Arbeitsspeicher der Launcher, jedes Spiel und jeder Server belegen (auf Wunsch auch auf der Startseite).
- **Discord Rich Presence:** Packname, Version, Spielzeit, Einzelspieler oder Server. Buttons für Freunde: **View modpack** (Modpack ansehen) und **Get ResinCraft** (ResinCraft holen).

## Instanzen

- **Ordnen:** Gruppen, eigene Reihenfolge (per Ziehen anordnen), Symbole, Banner und Farben.
- **Tabs:** Inhalte, Welten, Server, Screenshots, Notizen, Dateien (eingebauter Dateimanager und Editor), Logs und Einstellungen.
- **Aus anderer Instanz kopieren:** Welten, Mods und ihre Einstellungen (config-Ordner), Shaderpacks, Ressourcenpakete und Spieleinstellungen.
- **Duplizieren, exportieren** (Modrinth `.mrpack` oder CurseForge `.zip`) und **zu deinem eigenen Modpack machen**.
- **Befehle und Umgebung:** Befehle, die vor dem Start und nach dem Schließen des Spiels laufen, sowie Umgebungsvariablen.
- **Sofortige Schalter:** Einen Mod ein- oder auszuschalten geht sofort; das Umbenennen passiert im Hintergrund.
- **Sofort löschen und erstellen (neu):** Eine gelöschte Instanz verschwindet sofort und wandert im Hintergrund in den Papierkorb. Ein Modpack, das gerade installiert wird, erscheint direkt in der Bibliothek, mit dem Fortschritt auf seiner Karte.

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
- **Kein offizielles Server-Pack? (neu):** ResinCraft baut den Server aus der Modliste des Packs, sagt dir das beim Erstellen und zeigt das Abzeichen **Built by ResinCraft** (von ResinCraft gebaut).
- **Dateien zum selbst Herunterladen (neu):** Dateien, deren Autoren Downloads nur von ihrer Website erlauben, stehen im Tab **Console** (Konsole) des Servers. Vor jedem Start sucht ResinCraft sie in deinem Downloads-Ordner und in deinen Instanzen desselben Packs und legt sie an ihren Platz.
- **Arbeitsspeicher nach Packgröße (neu):** Ein neuer Server mit Mods bekommt ab 80 Mods 6 GB und ab 150 Mods 8 GB (höchstens die Hälfte des Arbeitsspeichers deines PCs).
- **Reine Client-Mods werden nur ausgeschaltet, wenn sie abstürzen:** Client-Mods bleiben auf dem Server, solange er normal startet. Stürzt ein Start ab, weil ein Mod nach dem Spiel-Client gegriffen hat, wird dieser Mod ausgeschaltet (`.jar.disabled`) und der Server startet von selbst neu. Ein Mod, den ein anderer Mod braucht, bleibt an.
- **Verwalten:** Konsole, Properties-Editor, Plugin- und Mod-Verwaltung mit Updates, Whitelist, Ops und Bans, Server-Icon, Backups.
- **Neu starten und Mehr-Menü:** Neu starten neben Stop; ein Menü mit Adresse kopieren, Ordner öffnen, sichern, Einstellungen und Löschen.
- **Online now** (gerade online): die Spieler auf deinem Server, alle paar Sekunden live geprüft.
- **Java flags** (Java-Flags): die abgestimmten Flags von ResinCraft oder die eigenen des Modpacks (`user_jvm_args.txt`, Startskripte).
- **Anti-xray (neu):** Paper-, Purpur-, Folia- und Leaf-Server können Erze vor X-Ray-Mods und Texturpaketen verstecken (Settings → Anti-xray).
- **Neu installieren (neu):** die Software eines Servers neu installieren oder seine Modpack-Dateien wieder ablegen. Welt und Einstellungen bleiben.
- **Sofort löschen und erstellen (neu):** Ein gelöschter Server verschwindet sofort; ein Server, der gerade erstellt wird, erscheint direkt in der Liste, mit seinem Fortschritt.
- **Fernsteuerung (neu):** Steuere deine Server aus jedem Browser oder vom Handy mit der Web-Konsole unter `console.rscraft.resincore.tech`, angemeldet mit demselben Minecraft-Konto, solange ResinCraft offen ist oder im Infobereich läuft (Einstellungen → Fernsteuerung). Sie kann Server starten, stoppen, beobachten und erstellen, kommt aber nie an deine Dateien oder andere Einstellungen.
- **Mit Freunden teilen:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - dein Router (UPnP)
  - deine eigene Domain, mit automatischem Cloudflare-DNS
- **Bedrock players can join** (Bedrock-Spieler können beitreten): Bedrock-Spieler kommen auch auf Java-Server (Geyser + Floodgate).
- **Whitelist requests** (Whitelist-Anfragen): Spieler fragen, ob sie beitreten dürfen, und du nimmst sie im Tab Players an oder lehnst sie ab.
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
- **Windows-Installer:** die `.exe`, eine `.msi` für verwaltete Installationen und **ResinCraft Setup (neu)**: ein kleiner Installer, der immer die neueste Version installiert, unter einem Link, der sich nie ändert.
- **Updates:** werden automatisch gesucht.
  - Ein **Update**-Button in der Titelleiste.
  - **Schließen und aktualisieren**, wenn Spiele oder Server laufen.
  - **Still aktualisieren** im Hintergrund.
- **Im Infobereich weiterlaufen:** Wenn du das Fenster schließt, laufen Server, der Discord-Bot, die Fernsteuerung und Downloads weiter.
- **ResinCraft neu installieren (neu):** Unter Einstellungen → Updates installiert sich die App über sich selbst neu, um sie zu reparieren. Instanzen, Konten und Einstellungen bleiben.
- **Designs (neu):** Dunkel, Hell, Obsidian, Tiefenschiefer, Kirsche, Nether, Das Ende, Halloween und Weihnachten, oder System.
  - Halloween schaltet sich vom 16. bis 31. Oktober von selbst ein, Weihnachten vom 10. bis 25. Dezember. Wählst du ein anderes Design, bleibt es deins; oder schalte die Feiertags-Themes aus.
  - **Eigenes CSS:** dein eigenes CSS über jedem Design, ausgehend von den aktuellen Farben.
  - Und eine versteckte Überraschung. Probier den Konami-Code.
- **16 Sprachen:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Speicher** verwalten (Daten auf ein anderes Laufwerk verschieben), **Sichern und Wiederherstellen** der Einstellungen.
