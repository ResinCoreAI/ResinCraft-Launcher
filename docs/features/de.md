# ResinCraft Launcher: Funktionen

Ein moderner, freundlicher Minecraft-Launcher von **Puirkle**, für Windows, macOS und Linux. Keine Telemetrie und kein ResinCraft-Konto.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Die Version in Klammern ist die, mit der die jeweilige Funktion kam. Die neuesten sind 1.4.0 und 1.4.1.

## Spielen

- **Instanzen (0.1.0):** getrennte Installationen pro Version oder Modpack, jede mit eigenen Mods, Welten, Java, Arbeitsspeicher und JVM-Argumenten.
- **Jede Minecraft-Version (0.1.0):** alle Releases und Snapshots, bis zurück zu den Classic-Alphas.
- **Mod-Loader (0.1.0):** Fabric, Quilt, Forge und NeoForge, mit Auswahl der Loader-Version. Klassisches Forge für 1.7.10 bis 1.12.2 (1.1.2), Forge bis zurück zu Minecraft 1.2.5 und LiteLoader (1.2.0). Auch OptiFine (1.0.1) und Jar-Mods (1.2.0).
- **Version und Loader ändern (1.2.7):** Minecraft-Version oder Mod-Loader einer Instanz im Tab Einstellungen wechseln, wie in Prism. Der Versions-Chip auf der Instanzseite führt direkt dorthin.
- **Spielen (0.1.0):** ein Klick. Mit **Diesmal spielen als** ein anderes Konto nehmen, oder mit **Zusätzlich spielen als** dieselbe Instanz zweimal mit zwei Konten starten (1.0.5).
- **Schnellbeitritt (1.1.0):** deine letzten Server und Einzelspielerwelten (1.2.3); ein Klick, und du bist direkt drin.
- **Welten (0.1.0):** eine Welt umbenennen, sichern oder eine Welt aus einer ZIP-Datei importieren. Eine Welt direkt spielen (1.2.0).
- **Automatische Weltsicherungen (1.4.0):** Solange ein Spiel oder Server läuft, werden seine Welten alle 30 Minuten gesichert; pro Welt bleiben die neuesten 3 (Einstellungen → Automatische Sicherung).
- **Sicherung wiederherstellen (1.4.0):** eine Welt aus einer beliebigen Sicherung zurückholen, deiner eigenen oder einer automatischen, oder sie als Kopie zurückholen. Die ersetzte Welt wandert in den Papierkorb.
- **Absturzhilfe (1.2.7):** Stürzt Minecraft ab, liest ResinCraft den Absturzbericht und die Logs. Es erklärt dir in einfachen Worten, was schiefgelaufen ist, und bietet eine Lösung mit einem Klick an. Lösungen: einen fehlenden Mod installieren, einen abgestürzten Mod ausschalten, das richtige Java wählen, dem Spiel mehr Arbeitsspeicher geben.
- **Neu installieren (1.3.0):** die Spieldateien einer Instanz erneut herunterladen oder die Dateien eines Modpacks wieder so ablegen, wie das Pack sie hat. Über das Menü der Instanz oder **Einstellungen → Reparieren**. Welten bleiben.
- **Abgestimmte Java-Flags (1.2.7):** Spiele starten mit den G1GC-Flags von ResinCraft für flüssigere Bilder (Einstellungen → Java, oder pro Instanz).
- **Java (0.1.0):** findet installierte Java-Versionen, wählt für jede Minecraft-Version die passende und lädt Eclipse Temurin herunter, wenn keine passt.
- **RAM-Anzeige (1.2.1):** wie viel Arbeitsspeicher der Launcher, jedes Spiel und jeder Server belegen (auf Wunsch auch auf der Startseite).
- **Discord Rich Presence (1.2.3):** Packname, Version, Spielzeit, Einzelspieler oder Server. Buttons für Freunde (1.2.6): **View modpack** (Modpack ansehen) und **Get ResinCraft** (ResinCraft holen).

## Instanzen

- **Ordnen:** Gruppen (0.2.0), eigene Reihenfolge (0.3.1, per Ziehen anordnen), Symbole und Banner (0.3.0) und Farben (0.1.0).
- **Tabs (0.1.0):** Inhalte, Welten, Server, Screenshots, Notizen, Dateien (eingebauter Dateimanager und Editor, 0.2.0), Logs und Einstellungen.
- **Aus anderer Instanz kopieren (1.2.5):** Welten, Shaderpacks, Ressourcenpakete und Spieleinstellungen; seit 1.2.6 auch Mods und ihre Einstellungen (config-Ordner).
- **Duplizieren (0.1.0), exportieren** (Modrinth `.mrpack` oder CurseForge `.zip`, 1.0.5) und **zu deinem eigenen Modpack machen (1.1.0)**.
- **Mehrere auswählen (1.1.0):** viele Instanzen auf einmal verschieben, löschen, duplizieren oder aktualisieren.
- **Befehle und Umgebung (1.2.0):** Befehle, die vor dem Start und nach dem Schließen des Spiels laufen, sowie Umgebungsvariablen.
- **Sofortige Schalter (1.2.7):** Einen Mod ein- oder auszuschalten geht sofort; das Umbenennen passiert im Hintergrund.
- **Sofort löschen und erstellen (1.3.0):** Eine gelöschte Instanz verschwindet sofort und wandert im Hintergrund in den Papierkorb. Ein Modpack, das gerade installiert wird, erscheint direkt in der Bibliothek, mit dem Fortschritt auf seiner Karte.

## Inhalte und Entdecken

- **Quellen (0.1.0):** Mods, Modpacks, Ressourcenpakete und Shader von Modrinth, CurseForge, FTB, ATLauncher, Technic und GitHub-Releases (0.3.1), gefiltert nach Version und Loader der Instanz.
- **Modpack-Seiten (1.2.0)** mit Beschreibung, Galerie, Versionen, Changelogs und **den Mods, die im Pack stecken (1.2.5)**.
- **Clever installieren (0.3.0):** jede beliebige Version wählen, mehrere auf einmal installieren, benötigte Abhängigkeiten kommen automatisch dazu, und derselbe Mod von zwei Seiten wird erkannt.
- **Updates (0.1.0):** Updates suchen und einspielen (einzeln oder alle); Modpack-Updates und Versionswechsel (1.1.0). **Alle aktualisieren** lädt bis zu vier gleichzeitig herunter (1.4.1).
- **Manuelle Downloads (1.0.1):** CurseForge-Dateien, deren Autoren Launcher blockieren, werden aus deinem Downloads-Ordner oder anderen **Überwachte Ordner** übernommen (1.0.5, Einstellungen → Downloads).
- **Listen (0.4.0):** die Mods, Ressourcenpakete oder Shader als Liste exportieren und eine Liste in eine andere Instanz installieren.
- **Server finden (1.4.0):** Entdecken → **Server** zeigt Server, die mit ResinCraft gehostet werden, und Server, die Spieler hinzugefügt haben. Suchen und filtern nach Kategorie, Region, Sprache, Modpack und Unterstützung für Bedrock oder Offline-Profile; liken und melden.
- **Eine Seite für jeden Server (1.4.1):** die vollständige Beschreibung und MOTD, alle Details, sein Client-Paket, Links und ein Feld für Whitelist-Anfragen.
- **Mit einem Klick beitreten (1.4.0):** **Beitreten** wählt die passende Instanz oder fragt nach. **Installieren & beitreten** erstellt eine Instanz mit genau den Mods des Servers und tritt bei.
- **Um die Whitelist bitten (1.4.0):** **Beitritt anfragen** schickt eine Anfrage, signiert mit deinem Minecraft-Konto. Dort steht dann **Waiting for the host** (warten auf den Host), **On the whitelist** (auf der Whitelist) oder **Ask again** (erneut fragen), und ResinCraft sagt dir Bescheid (und lässt die Taskleiste blinken), sobald der Host antwortet (1.4.1).

## Server hosten

- **Servertypen (0.1.0):** Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge und NeoForge; seit 0.3.1 auch Leaf, Arclight (Mods und Plugins zusammen) und Bedrock Dedicated Server.
- **Aus einem Modpack oder einer Instanz (0.1.0):** einen Server aus Modrinth, CurseForge (offizielles Server-Pack, falls es eins gibt) oder FTB erstellen, oder aus einer deiner Instanzen.
- **Kein offizielles Server-Pack? (1.2.11):** ResinCraft baut den Server aus der Modliste des Packs, sagt dir das beim Erstellen und zeigt das Abzeichen **Built by ResinCraft** (von ResinCraft gebaut).
- **Dateien zum selbst Herunterladen (1.2.9):** Dateien, deren Autoren Downloads nur von ihrer Website erlauben, stehen im Tab **Console** (Konsole) des Servers. Vor jedem Start sucht ResinCraft sie in deinem Downloads-Ordner und in deinen Instanzen desselben Packs und legt sie an ihren Platz.
- **Arbeitsspeicher nach Packgröße (1.2.9):** Ein neuer Server mit Mods bekommt ab 80 Mods 6 GB und ab 150 Mods 8 GB (höchstens die Hälfte des Arbeitsspeichers deines PCs).
- **Reine Client-Mods werden nur ausgeschaltet, wenn sie abstürzen (1.2.8):** Client-Mods bleiben auf dem Server, solange er normal startet. Stürzt ein Start ab, weil ein Mod nach dem Spiel-Client gegriffen hat, wird dieser Mod ausgeschaltet (`.jar.disabled`) und der Server startet von selbst neu. Ein Mod, den ein anderer Mod braucht, bleibt an.
- **Verwalten (0.1.0):** Konsole, Properties-Editor, Plugin- und Mod-Verwaltung mit Updates (0.3.0), Whitelist, Ops und Bans, Server-Icon (1.2.5), Backups.
- **Schnellere Updates (1.4.1):** **Alle aktualisieren** lädt bis zu vier Plugins oder Mods gleichzeitig herunter und spielt sie zusammen ein, oder merkt sie für den nächsten Neustart vor, während der Server läuft.
- **Deinen Server eintragen (1.4.0):** Settings → **Serverliste** bringt ihn in Entdecken → Server, mit Beschreibung, Tags, Region und Sprache. Server, die du woanders betreibst, kannst du mit **Server einreichen** hinzufügen.
- **Client-Pakete (1.4.0):** veröffentliche, was Spieler zum Beitreten brauchen. Seit 1.4.1: das Modpack des Servers (von seiner eigenen Seite installiert), jeden weiteren Mod von Modrinth oder CurseForge und deine eigenen Dateien direkt von deinem PC; Ressourcenpakete und Shader gleich mit; auch für Plugin- und Vanilla-Server.
- **Neu starten und Mehr-Menü (1.2.7):** Neu starten neben Stop; ein Menü mit Adresse kopieren, Ordner öffnen, sichern, Einstellungen und Löschen.
- **Online now** (gerade online, 1.2.7): die Spieler auf deinem Server, alle paar Sekunden live geprüft.
- **Java flags** (Java-Flags, 0.2.0): die abgestimmten Flags von ResinCraft oder die eigenen des Modpacks (`user_jvm_args.txt`, Startskripte, 1.2.7).
- **Anti-xray (1.3.0):** Paper-, Purpur-, Folia- und Leaf-Server können Erze vor X-Ray-Mods und Texturpaketen verstecken (Settings → Anti-xray).
- **Anti-cheat (1.4.0):** Ein Schalter installiert den Anti-Cheat Grim auf Plugin-Servern.
- **Welt neu erstellen (1.4.0):** einen Server mit einer neuen Welt neu beginnen; die alte wandert in den Papierkorb.
- **Neu installieren (1.3.0):** die Software eines Servers neu installieren oder seine Modpack-Dateien wieder ablegen. Welt und Einstellungen bleiben.
- **Sofort löschen und erstellen (1.3.0):** Ein gelöschter Server verschwindet sofort; ein Server, der gerade erstellt wird, erscheint direkt in der Liste, mit seinem Fortschritt.
- **Fernsteuerung (1.3.0):** Steuere deine Server aus jedem Browser oder vom Handy mit der Web-Konsole unter `console.rscraft.resincore.tech`, angemeldet mit demselben Minecraft-Konto, solange ResinCraft offen ist oder im Infobereich läuft (Einstellungen → Fernsteuerung). Sie kann alles, was die Serververwaltung kann. Dateimanager, Java-Einstellungen und Plugins von beliebigen Links brauchen einen zweiten Schalter, **Volle Kontrolle erlauben**. Ein **Web-Konsole**-Button auf der Seite Servers (1.3.1).
- **Deine anderen PCs (1.4.1):** Das PC-Menü im Tab Servers öffnet die Server auf deinen anderen PCs, auf denen die Fernsteuerung an ist: starten, stoppen, Spieler, Plugins und Einstellungen verwalten und dort Server erstellen oder importieren.
- **Freunde helfen lassen (1.4.1):** Einstellungen → Fernsteuerung → **People who can use your servers** (Leute, die deine Server nutzen dürfen): Gib einem Minecraft-Konto **Viewer** (Zuschauer), **Operator** oder **Manager** (Verwalter) als Zugriff, für alle deine Server oder die, die du auswählst. Dateien, Java-Einstellungen und das Erstellen oder Löschen von Servern bleiben deine Sache.
- **Mit Freunden teilen:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`, 0.3.0)
  - playit.gg (0.2.0)
  - bore (0.2.0)
  - dein Router (UPnP, 0.2.0)
  - deine eigene Domain, mit automatischem Cloudflare-DNS (0.2.0), bestätigt über einen TXT-Eintrag (**Verifizierung ausstehend**, **Verifiziert**, 1.3.1)
  - **Automatic** (automatisch) für neue Server (1.3.2): zuerst dein Router, dann ResinCraft Relay, dann playit.gg
- **Bedrock players can join** (Bedrock-Spieler können beitreten, 0.3.1): Bedrock-Spieler kommen auch auf Java-Server (Geyser + Floodgate).
- **Whitelist requests** (Whitelist-Anfragen, 1.2.7): Spieler fragen, ob sie beitreten dürfen, und du nimmst sie im Tab Players an oder lehnst sie ab.
  - **Web page** (Webseite): unter der eigenen Adresse deines Servers oder deiner verifizierten Domain (1.3.2, mit ResinCraft Relay).
  - **Your Discord bot** (dein eigener Discord-Bot): Spieler tippen `/whitelist`.
  - **Die Serverliste (1.4.0):** Spieler mit ResinCraft fragen über Entdecken → Server.
  - **Deine Antwort:** Accept (annehmen) setzt sie auf die Whitelist; der Bot sagt es ihnen per Direktnachricht.
  - **Schutz:** Gebannte Spieler werden abgelehnt; **Reject and block** (ablehnen und blockieren) verhindert, dass jemand erneut fragt, mit einer Liste **Blocked** (blockiert) und **Unblock** (entsperren) für jeden Spieler (1.4.1).

## Bedrock Edition (Windows)

- **Bedrock-Seite (0.3.1):** Minecraft für Windows und Minecraft Preview installieren (1.0.0), aktualisieren und reparieren (1.2.0), starten und deinstallieren (1.2.3) (die Welten werden vorher gesichert).

## Konten

- **Microsoft-Anmeldung (0.1.0)**, mit verschlüsselt gespeicherten Tokens. **Offline-Profile (0.1.0)** für Einzelspieler und LAN. **Bedrock-Profile (1.0.0)** für Microsoft-Konten ohne Java Edition.
- **Skins, Umhänge und Namensänderung (0.1.0).** Eine Skin-Bibliothek, Skins über einen Spielernamen und eine Online-Suche (1.0.3); eine 3D-Skin-Ansicht (1.1.0).

## Von einem anderen Launcher umziehen

- **Import (1.0.5):** Instanzen aus Prism Launcher, MultiMC, der Modrinth App, der CurseForge-App, ATLauncher, GDLauncher und dem offiziellen Minecraft Launcher, samt Mods, Welten, Packs und Einstellungen. Auch Server lassen sich importieren.
- **Auf einen anderen PC umziehen (1.1.1):** Einstellungen → Auf einen anderen PC umziehen speichert alles in einer `.resincraft`-Datei; öffne sie auf dem neuen PC.

## Die App

- **Windows (0.1.0), macOS** (Apple Silicon und Intel) **und Linux** (x64 und ARM64: AppImage, `.deb`, 1.2.0; `.tar.gz`, 1.2.4).
- **Windows-Installer:** die `.exe` (0.1.0), eine `.msi` für verwaltete Installationen (1.2.10) und **ResinCraft Setup (1.3.0)**: ein kleiner Installer, der immer die neueste Version installiert, unter einem Link, der sich nie ändert.
- **Updates (0.3.0):** werden automatisch gesucht.
  - Ein **Update**-Button in der Titelleiste (1.2.7).
  - **Schließen und aktualisieren**, wenn Spiele oder Server laufen (1.2.7).
  - **Still aktualisieren** im Hintergrund (1.2.7).
- **Im Infobereich weiterlaufen (1.2.7):** Wenn du das Fenster schließt, laufen Server, der Discord-Bot, die Fernsteuerung und Downloads weiter.
- **ResinCraft neu installieren (1.3.0):** Unter Einstellungen → Updates installiert sich die App über sich selbst neu, um sie zu reparieren. Instanzen, Konten und Einstellungen bleiben.
- **Designs:** Dunkel, Hell und System (0.1.0); Obsidian, Tiefenschiefer, Kirsche, Nether und Das Ende (1.3.0).
  - **Theme erstellen (1.3.0):** dein eigenes Design, mit **Feinabstimmung** für jede Farbe (Farbwähler) und CSS für Teile der App (1.3.2).
  - **Designs teilen (1.3.2):** **Exportieren** als `.rctheme`-Datei, mit Bild und CSS; **Theme importieren** oder die Datei doppelklicken (1.4.0).
  - **Eigenes CSS (1.3.0):** dein eigenes CSS über jedem Design, ausgehend von den aktuellen Farben, mit eingebauter Anleitung zu den Farben, den Teilen der App und fertigen Beispielen.
  - Und eine versteckte Überraschung (1.3.0). Probier den Konami-Code.
- **16 Sprachen (0.1.0):** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Speicherverwaltung (1.1.0):** Instanzen auf einem anderen Laufwerk aufbewahren. **Instanzen verschieben** dorthin oder **Ohne Verschieben verwenden** (1.4.1).
- **Einstellungen zurücksetzen** und **ResinCraft deinstallieren (1.4.0)** in den Einstellungen.
