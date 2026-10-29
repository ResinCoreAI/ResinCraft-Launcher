# ResinCraft Launcher: Benutzerhandbuch

So erledigst du die häufigsten Dinge in ResinCraft. **Fett** gedruckte Namen siehst du genau so auf dem Bildschirm. Ein paar Teile der Server-Seite sind noch nicht übersetzt; dort zeigt die App Englisch, deshalb stehen diese Namen hier auch auf Englisch (mit Übersetzung in Klammern). Jeder Abschnitt nennt die Version, mit der er kam.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Inhalt

1. [Installieren und anmelden](#1-installieren-und-anmelden)
2. [Deine erste Instanz erstellen](#2-deine-erste-instanz-erstellen)
3. [Mods, Ressourcenpakete und Shader hinzufügen](#3-mods-ressourcenpakete-und-shader-hinzufügen)
4. [Ein Modpack installieren](#4-ein-modpack-installieren)
5. [Minecraft-Version oder Mod-Loader ändern](#5-minecraft-version-oder-mod-loader-ändern)
6. [Wenn Minecraft abstürzt](#6-wenn-minecraft-abstürzt)
7. [Inhalte aus einer anderen Instanz kopieren](#7-inhalte-aus-einer-anderen-instanz-kopieren)
8. [Einen Server hosten](#8-einen-server-hosten)
9. [Freunde auf deinen Server lassen](#9-freunde-auf-deinen-server-lassen)
10. [Whitelist-Anfragen (Webseite und Discord-Bot)](#10-whitelist-anfragen-webseite-und-discord-bot)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Von einem anderen Launcher umziehen](#12-von-einem-anderen-launcher-umziehen)
13. [Updates und der Infobereich](#13-updates-und-der-infobereich)
14. [Designs und eigenes CSS](#14-designs-und-eigenes-css)
15. [Fernsteuerung (Web-Konsole)](#15-fernsteuerung-web-konsole)
16. [Neu installieren und reparieren](#16-neu-installieren-und-reparieren)
17. [Server finden und beitreten](#17-server-finden-und-beitreten)
18. [Deinen Server eintragen und seine Mods teilen](#18-deinen-server-eintragen-und-seine-mods-teilen)
19. [Deine anderen PCs und Freunde, die helfen](#19-deine-anderen-pcs-und-freunde-die-helfen)
20. [Weltsicherungen](#20-weltsicherungen)
21. [Speicher und Umzug auf einen anderen PC](#21-speicher-und-umzug-auf-einen-anderen-pc)
22. [Fehlerbehebung](#22-fehlerbehebung)

## 1. Installieren und anmelden

*Seit Version 0.1.0.*

1. Lade ResinCraft für dein System von der [Releases-Seite](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest) herunter:
   - **Windows:** [`ResinCraft-Setup.exe`](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/download/setup/ResinCraft-Setup.exe) (installiert immer die neueste Version), oder `…_amd64_windows.exe` / `…_amd64_windows.msi`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` oder `.tar.gz`
2. Installiere und öffne es. Windows meldet eventuell, dass der Herausgeber nicht verifiziert ist; unter macOS klickst du beim ersten Mal mit der rechten Maustaste auf die App und wählst **Öffnen**.
3. Öffne **Konten** → **Mit Microsoft anmelden** und melde dich auf Microsofts eigener Seite an. Dein Passwort geht nie durch ResinCraft.
   - Kein Microsoft-Konto? Füge ein **Offline-Profil** für Einzelspieler und LAN hinzu.

## 2. Deine erste Instanz erstellen

*Seit Version 0.1.0.*

Eine *Instanz* ist eine eigene Minecraft-Installation mit eigenen Mods, Welten und Einstellungen.

1. Öffne **Bibliothek** → **Neue Instanz**.
2. Wähle eine **Minecraft-Version** und einen **Mod-Loader**:
   - **Vanilla:** keine Mods.
   - **Fabric** oder **Quilt:** leichte Mods, Performance-Mods.
   - **Forge** oder **NeoForge:** große Content-Mods.
3. Gib ihr einen Namen und klicke auf **Instanz erstellen**.
4. Klicke auf **Spielen**. Beim ersten Start werden das Spiel und das passende Java heruntergeladen; danach geht der Start schnell.

Tipp: **Schnellbeitritt** links zeigt deine letzten Server und Welten; ein Klick, und du bist direkt drin.

## 3. Mods, Ressourcenpakete und Shader hinzufügen

*Seit Version 0.1.0.*

1. Öffne deine Instanz → Tab **Inhalte** → **Mods hinzufügen**. Oder öffne **Entdecken** in der linken Leiste.
2. Suche, oder stöbere auf Modrinth, CurseForge, FTB und den anderen Seiten. Es werden nur Versionen angezeigt, die zu deiner Instanz passen.
3. Klicke auf **Installieren**. Benötigte Mods (Abhängigkeiten) kommen von selbst dazu.
4. Zurück in **Inhalte**:
   - mit dem Schalter neben einem Mod schaltest du ihn ein oder aus;
   - mit **Nach Updates suchen** aktualisierst du Mods (einzeln oder alle). **Alle aktualisieren** lädt bis zu vier gleichzeitig herunter.

Manche CurseForge-Autoren erlauben Launchern nicht, ihre Dateien herunterzuladen. ResinCraft listet sie dann unter **Alle herunterladen** auf: Lade sie in deinem Browser herunter, und ResinCraft findet sie von selbst in deinem Downloads-Ordner. Speichert dein Browser woanders? Füge diesen Ordner unter **Einstellungen → Downloads → Überwachte Ordner** hinzu.

## 4. Ein Modpack installieren

*Seit Version 0.1.0.*

1. Öffne **Entdecken** → **Modpacks** (oder **Bibliothek** → **Neue Instanz** → **Modpack herunterladen**).
2. Öffne ein Pack, um seine Beschreibung, seine Versionen und die enthaltenen **Mods** zu sehen.
3. Klicke auf **Pack installieren**. Die neue Instanz erscheint sofort in der **Bibliothek**, mit dem Installationsfortschritt auf ihrer Karte; **Spielen** funktioniert, sobald sie fertig ist.
4. Bekommt das Pack ein Update, sagt ResinCraft dir Bescheid; du kannst auf der Instanzseite auch eine andere Pack-Version wählen.

## 5. Minecraft-Version oder Mod-Loader ändern

*Seit Version 1.2.7.*

1. Klicke auf der Instanzseite oben auf den Versions-Chip (zum Beispiel **1.21.4 · Fabric 0.16.10**). Er öffnet **Einstellungen → Version**.
2. Wähle die **Minecraft**-Version, den **Mod-Loader** und die Loader-Version.
3. Klicke auf **Version ändern** und bestätige. Beim nächsten Start wird heruntergeladen, was nötig ist.
4. Mods für die alte Version laufen eventuell nicht mehr: Geh zu **Inhalte** und klicke auf **Nach Updates suchen**.

## 6. Wenn Minecraft abstürzt

*Seit Version 1.2.7.*

Stürzt das Spiel ab, öffnet sich das Fenster **Minecraft ist abgestürzt**. ResinCraft liest den Absturzbericht und die Logs und sagt dir, was schiefgelaufen ist. Diese Meldungen zeigt die App noch auf Englisch, zum Beispiel:

- *Sodium Extra needs Sodium, which isn't installed* (Sodium Extra braucht Sodium, das fehlt) → **Beheben** installiert Sodium.
- *citresewn crashed the game* (citresewn hat das Spiel abstürzen lassen) → **Beheben** schaltet diesen Mod aus (später in **Inhalte** wieder einschalten).
- *This needs Java 21 or newer* (braucht Java 21 oder neuer) → **Beheben** lässt ResinCraft das passende Java wählen.
- *Minecraft ran out of memory* (Minecraft hatte zu wenig Arbeitsspeicher) → **Beheben** gibt ihm mehr Speicher.

Klicke bei jedem Punkt auf **Beheben** oder gleich auf **Alles beheben und spielen**. Es ändert sich nichts, bevor du einen Button drückst. Findet ResinCraft die Ursache nicht, zeigt dir **Protokoll öffnen**, was das Spiel zuletzt gemeldet hat.

## 7. Inhalte aus einer anderen Instanz kopieren

*Seit Version 1.2.5.*

1. Öffne die Instanz, *in die* du kopieren willst → klicke oben auf den Button **Aus anderer Instanz kopieren**.
2. Wähle die Instanz, *aus der* du kopierst.
3. Hake an, was du haben willst:
   - **Welten**;
   - **Mods** und **Mod-Einstellungen** (der config-Ordner);
   - **Shaderpacks** und **Ressourcenpakete**;
   - **Spieleinstellungen** (Grafik, Steuerung, Tastenbelegung).
4. Klicke auf **Kopieren**. Welten, deren Name schon vergeben ist, bekommen einen neuen Namen. Mods und Packs, die du schon hast, bleiben, wie sie sind.

## 8. Einen Server hosten

*Seit Version 0.1.0.*

1. Öffne **Servers** → **Create server** (Server erstellen).
2. Wähle, wie:
   - **Server software** (Server-Software): Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack:** dasselbe Pack, das deine Freunde spielen.
   - **From an instance** (aus einer Instanz): die Mods, die du schon benutzt.
   - **From a file** (aus einer Datei).
3. Klicke auf **Create** (Erstellen). Das Fenster schließt sich, und der Download läuft unten rechts weiter.
4. Stimme der Minecraft-EULA zu, wenn du gefragt wirst, und klicke dann auf **Start**.

Auf der Seite eines Servers:

- **Console** (Konsole): was der Server meldet; unten tippst du Befehle ein.
- **Players** (Spieler): wer gerade online ist, dazu Whitelist, Operatoren und Bans.
- **Mods** oder **Plugins:** hinzufügen, aktualisieren oder ausschalten. **Alle aktualisieren** lädt bis zu vier gleichzeitig herunter; läuft der Server, werden die Updates beim nächsten Neustart eingespielt.
- **Properties** (Eigenschaften): die Servereinstellungen (Schwierigkeit, Whitelist an/aus und so weiter).
- **Settings** (Einstellungen):
  - Arbeitsspeicher, Java und **Java flags** (die von ResinCraft oder die eigenen des Modpacks);
  - wie Freunde sich verbinden;
  - das Server-Icon.
- **Neu starten** und das **⋯**-Menü (Adresse kopieren, Ordner öffnen, jetzt sichern, löschen).

- **Settings → Anti-xray** (Einstellungen → Anti-Xray; Paper, Purpur, Folia, Leaf): Erze vor X-Ray-Mods und Texturpaketen verstecken. **Hide ores** (Erze verstecken) belastet den Server kaum; **Fake ores** (falsche Erze) ist stärker. Es wirkt ab dem nächsten Start.
- **Settings → Anti-cheat** (Einstellungen → Anti-Cheat; Plugin-Server): Ein Schalter installiert Grim.

Mods, die nur im Spiel-Client laufen (Shader, Minimaps…), bleiben auf dem Server, solange er normal startet. Bringt einer ihn zum Absturz, schaltet ResinCraft diesen Mod aus und startet den Server neu; im Tab **Mods** kannst du ihn wieder einschalten.

Hat ein Modpack kein offizielles Server-Pack, baut ResinCraft den Server aus der Modliste des Packs: Das Fenster nach **Create** (Erstellen) sagt dir das, und der Server zeigt das Abzeichen **Built by ResinCraft** (von ResinCraft gebaut). Manche Pack-Dateien lassen sich nur von der Website ihres Autors herunterladen. Sie stehen oben im Tab **Console**, mit **Get file** (Datei holen). Lade sie in deinem Browser herunter, und ResinCraft legt sie vor dem nächsten Start an ihren Platz, oder klicke auf **Look again** (erneut suchen).

## 9. Freunde auf deinen Server lassen

*Seit Version 0.2.0.*

Auf der Server-Seite unter **Settings** → **How friends connect** (wie Freunde sich verbinden):

- **Automatic** (automatisch, empfohlen und Standard für neue Server): zuerst dein Router, dann **ResinCraft Relay**, dann playit.gg.
- **ResinCraft Relay:** Freunde verbinden sich mit `yourname.rscraft.resincore.tech`. Dafür muss ein Microsoft-Konto angemeldet sein.
- **playit.gg:** eine kostenlose Adresse, die gleich bleibt. Beim ersten Start wirst du gebeten, ein playit.gg-Konto zu verknüpfen (ein Gastkonto reicht).
- **Port selbst weitergeleitet:** wenn du deinen Router selbst eingerichtet hast.

Im Tab **Console** steht die Adresse, die du deinen Freunden gibst. Auch Freunde mit Bedrock können auf Java-Server: Schalte **Bedrock players can join** (Bedrock-Spieler können beitreten) ein.

**Eigene Domain:** Schalte mit ResinCraft Relay **Eigene Domain mit ResinCraft Relay verbinden** ein und gib die Domain ein. Lege die angezeigten DNS-Einträge an, auch den TXT-Eintrag, der beweist, dass die Domain dir gehört, und klicke dann auf **Verifizieren**. Das Abzeichen wechselt von **Verifizierung ausstehend** zu **Verifiziert**. Ist das automatische Cloudflare-DNS an, legt ResinCraft die Einträge für dich an.

## 10. Whitelist-Anfragen (Webseite und Discord-Bot)

*Seit Version 1.2.7.*

Lass Spieler fragen, ob sie beitreten dürfen, und entscheide selbst, wen du annimmst oder ablehnst.

1. Setze in **Properties** den Eintrag **white-list** auf **true** (sonst kann sowieso jeder beitreten).
2. Öffne den Tab **Players** des Servers und schalte **Whitelist requests** (Whitelist-Anfragen) ein.
3. Wähle, wie Spieler fragen:
   - **Web page** (Webseite):
     - Funktioniert, wenn der Server über ResinCraft Relay geteilt wird.
     - Klicke auf **Copy link** (Link kopieren) und schick den Spielern den Link. Es ist die eigene Adresse deines Servers oder deine verifizierte Domain.
     - Die Seite funktioniert, solange der Server online ist.
   - **Your Discord bot** (dein Discord-Bot):
     1. Öffne das [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token** und kopiere den Token.
     2. Füge ihn in ResinCraft ein und klicke auf **Save** (Speichern). Er wird sicher auf deinem PC verwahrt und nie wieder angezeigt.
     3. Sobald dort **Online as …** steht, klicke auf **Add the bot to your Discord** (Bot zu deinem Discord hinzufügen) und wähle deinen Discord-Server.
     4. Spieler tippen in deinem Discord `/whitelist TheirName` (mit ihrem eigenen Minecraft-Namen).
4. Anfragen erscheinen im Tab **Players**:
   - **Accept** (annehmen): setzt den Spieler auf die Whitelist (der Bot sagt es ihm per Direktnachricht).
   - **Reject** (ablehnen): lehnt die Anfrage ab.
   - **Reject and block** (ablehnen und blockieren): lehnt ab und verhindert, dass diese Person erneut fragt. Blockierte Spieler stehen in der Liste **Blocked** (blockiert), jeweils mit **Unblock** (entsperren).
   - **Aus der Serverliste:** Spieler mit ResinCraft können auf der Seite deines Servers unter Entdecken → Server fragen (Abschnitt 17).

Anfragen von gebannten Spielern werden automatisch abgelehnt. Der Discord-Bot arbeitet, solange ResinCraft offen ist; schalte den Infobereich ein (Abschnitt 13), damit er auch bei geschlossenem Fenster weiterläuft.

## 11. Bedrock Edition

*Seit Version 0.3.1.*

Unter Windows installiert, öffnet und deinstalliert **Bedrock** in der linken Leiste Minecraft für Windows und Minecraft Preview. Vor dem Deinstallieren werden deine Welten gesichert.

## 12. Von einem anderen Launcher umziehen

*Seit Version 1.0.5.*

**Bibliothek** → **Von einem anderen Launcher umziehen** holt Instanzen aus:

- Prism Launcher und MultiMC
- der Modrinth App und der CurseForge-App
- ATLauncher und GDLauncher
- dem offiziellen Minecraft Launcher

Ihre Mods, Welten und Einstellungen kommen mit. Die Originale bleiben, wo sie sind.

## 13. Updates und der Infobereich

*Seit Version 0.3.0 (der Infobereich seit 1.2.7).*

- **Updates:**
  - Ist eine neue ResinCraft-Version bereit, erscheint in der Titelleiste ein **Update**-Button.
  - Läuft gerade ein Spiel oder Server, wählst du **Schließen und aktualisieren** (Server speichern vorher ihre Welten) oder **Aktualisieren, sobald sie gestoppt sind**.
- **Still aktualisieren** (Einstellungen → Updates): installiert neue Versionen im Hintergrund, ohne zu fragen, sobald nichts mehr läuft.
- **Im Infobereich weiterlaufen** (Einstellungen → Darstellung):
  - Wenn du das Fenster schließt, läuft ResinCraft weiter: Server, der Discord-Bot und Downloads arbeiten weiter.
  - Klicke auf das Symbol im Infobereich, um es wieder zu öffnen; Rechtsklick → **Quit ResinCraft** (ResinCraft beenden) beendet es wirklich.

## 14. Designs und eigenes CSS

*Seit Version 1.3.0.*

1. Öffne **Einstellungen → Darstellung → Design** und wähle eine Karte: **System**, Dunkel, Hell, Obsidian, Tiefenschiefer, Kirsche, Nether oder Das Ende.
2. **Eigenes CSS:** Klicke auf **Aktuelle Farben einfügen**, ändere beliebige Farben und dann auf **Anwenden**. Es wirkt über jedem Design; **Leeren** entfernt es.
3. Öffne **So passt du es an** unter dem Feld: Dort stehen die Farben und Formen, die du ändern kannst, die Teile der App, die du gestalten kannst, und Beispiele (ein Hintergrundbild, rundere Ecken, größere Schrift …). Klicke auf eins, um es zu deinem CSS hinzuzufügen.

Psst: Probier den Konami-Code (↑ ↑ ↓ ↓ ← → ← → B A) oder klicke siebenmal auf das ResinCraft-Logo.

## 15. Fernsteuerung (Web-Konsole)

*Seit Version 1.3.0.*

Steuere deine Server aus jedem Browser oder vom Handy.

1. Melde dich unter **Konten** mit einem Microsoft-Konto an.
2. Schalte **Einstellungen → Fernsteuerung → Fernsteuerung erlauben** ein. Steht dort **Verbunden als …**, bist du bereit.
3. Schalte **Im Infobereich weiterlaufen** ein (Abschnitt 13), damit es auch nach dem Schließen des Fensters weiterläuft.
4. Öffne `https://console.rscraft.resincore.tech` (oder klicke auf **Web-Konsole öffnen**) und melde dich mit **demselben Minecraft-Konto** an.

In der Konsole kannst du alles, was die Serververwaltung kann: Server starten, stoppen und neu starten, die Konsole verfolgen und Befehle eintippen, Spieler und Whitelist-Anfragen verwalten, Plugins und Mods hinzufügen und aktualisieren, Properties und Einstellungen ändern, die Modpack-Version wechseln, neu installieren, sichern und neue Server erstellen. Jede Änderung erscheint in ResinCraft als Hinweis.

Dateimanager, Java-Einstellungen und Plugins von beliebigen Download-Links können Programme auf deinem PC ausführen, deshalb brauchen sie einen zweiten Schalter: **Volle Kontrolle erlauben**. Schalte ihn nur ein, wenn dein Minecraft-(Microsoft-)Konto die zweistufige Überprüfung nutzt. Ohne ihn kommt die Konsole nie an etwas anderes auf deinem PC.

## 16. Neu installieren und reparieren

*Seit Version 1.3.0.*

- **Eine Instanz startet nicht, weil Dateien fehlen oder kaputt sind:**
  - Rechtsklick auf sie in der **Bibliothek** → **Neu installieren** → **Spieldateien**, oder öffne ihre **Einstellungen → Reparieren**.
  - Minecraft, der Mod-Loader, Bibliotheken und Assets werden erneut heruntergeladen. Welten, Mods und Einstellungen bleiben.
- **Die Dateien eines Modpacks wurden geändert oder gelöscht:** **Neu installieren** → **Modpack-Dateien** legt die Mods, Configs und Skripte des Packs wieder ab. Geänderte Dateien werden vorher gesichert; Welten und deine eigenen Mods bleiben.
- **Ein Server:** sein **⋯**-Menü → **Neu installieren** → **Server-Software** (oder **Modpack-Dateien** bei einem Modpack-Server). Welt, Einstellungen und Spielerlisten bleiben.
- **ResinCraft selbst:** **Einstellungen → Updates → ResinCraft neu installieren** installiert die App über sich selbst neu.

## 17. Server finden und beitreten

*Seit Version 1.4.0.*

1. Öffne **Entdecken** → **Server**. Dort stehen Server, die mit ResinCraft gehostet werden, und Server, die Spieler hinzugefügt haben.
2. Suche, oder filtere nach Kategorie (SMP, PvP, Minispiele, Skyblock…), Region, Sprache, Modpack und danach, ob Bedrock-Spieler oder Offline-Profile beitreten können.
3. Klicke auf das Icon oder den Namen eines Servers, um seine Seite zu öffnen: die vollständige Beschreibung, alle Details, sein Client-Paket und Links.
4. Klicke auf **Beitreten**. ResinCraft wählt die Instanz, die zum Server passt, oder fragt, welche du nehmen willst. Ein Server mit eigenen Mods hat **Installieren & beitreten**: Das erstellt eine Instanz mit genau diesen Mods und tritt bei.
5. Server mit Whitelist? Klicke auf **Beitritt anfragen**. Der Button zeigt **Waiting for the host** (warten auf den Host), dann **On the whitelist** (auf der Whitelist; klicke auf **Beitreten**) oder **Ask again** (erneut fragen). ResinCraft sagt dir Bescheid und lässt die Taskleiste blinken, sobald der Host antwortet.

Gib Servern, die dir gefallen, ein Like, und melde die, die gegen die Regeln verstoßen.

## 18. Deinen Server eintragen und seine Mods teilen

*Seit Version 1.4.0.*

1. Schalte in den **Settings** (Einstellungen) eines gehosteten Servers **Serverliste** ein (standardmäßig ist sie aus).
2. Füge eine Beschreibung, Tags, Region und Sprache hinzu. Die Unterstützung für Bedrock und Offline-Profile sowie das Modpack werden von selbst ausgefüllt.
3. **Client-Paket:** Veröffentliche, was Spieler brauchen, damit sie **Installieren & beitreten** nutzen können:
   - das Modpack des Servers, von seiner eigenen Seite installiert;
   - jeden weiteren Mod, von Modrinth oder CurseForge;
   - deine eigenen Dateien, direkt von deinem PC gesendet, solange ResinCraft läuft;
   - Ressourcenpakete und Shader, die du mitschicken willst.
   Auch Plugin- und Vanilla-Server können eins haben: Wähle eine Instanz mit der Minecraft-Version des Servers.

Ein Server, den du woanders betreibst? Füge ihn mit **Server einreichen** unter Entdecken → Server hinzu.

## 19. Deine anderen PCs und Freunde, die helfen

*Seit Version 1.4.1.*

**Deine anderen PCs:** Schalte auf jedem PC die Fernsteuerung ein (Abschnitt 15) und melde dich mit demselben Minecraft-Konto an. Öffne dann in **Servers** oben das PC-Menü und wähle einen PC. Von hier aus kannst du seine Server starten, stoppen und verwalten und dort auch **Create server** (Server erstellen) oder **Server importieren** nutzen. Um aus einem Ordner auf diesem PC zu importieren, gib den Ordner ein; dafür muss auf diesem PC **Volle Kontrolle erlauben** an sein.

**Freunde, die beim Betreiben deiner Server helfen:**

1. Öffne **Einstellungen → Fernsteuerung → People who can use your servers** (Leute, die deine Server nutzen dürfen).
2. Gib ihren Minecraft-Namen ein und klicke auf **Add** (hinzufügen).
3. Wähle, was sie dürfen:
   - **Viewer** (Zuschauer): Konsole, Spieler und Einstellungen sehen.
   - **Operator:** zusätzlich starten, stoppen und neu starten, Befehle senden und Spieler und Whitelist-Anfragen verwalten.
   - **Manager** (Verwalter): zusätzlich Einstellungen, Plugins und Mods sowie das Modpack ändern.
4. Lass **All servers** (alle Server) stehen, oder wähle die Server unter **Which servers?** (welche Server?).

Sie öffnen deine Server aus ihrem ResinCraft (über das PC-Menü in **Servers**) oder über die Web-Konsole, solange die Fernsteuerung an ist und ResinCraft auf deinem PC läuft. Dateien, Java-Einstellungen und das Erstellen oder Löschen von Servern bleiben deine Sache.

## 20. Weltsicherungen

*Seit Version 1.4.0.*

- **Automatisch:** Solange ein Spiel oder Server läuft, werden seine Welten alle 30 Minuten gesichert; pro Welt bleiben die neuesten 3. Ändern oder ausschalten kannst du das unter **Einstellungen → Automatische Sicherung**.
- **Wiederherstellen:** Klicke im Tab **Welten** einer Instanz, oder bei einem gehosteten Server unter **Settings → Backups** (Einstellungen → Sicherungen), bei einer Sicherung auf **Wiederherstellen**, egal ob deine eigene oder eine automatische. Die ersetzte Welt wandert in den Papierkorb, so lässt sich eine Wiederherstellung rückgängig machen. Eine Einzelspielerwelt kann auch als Kopie zurückkommen.

## 21. Speicher und Umzug auf einen anderen PC

*Seit Version 1.1.0 (Auf einen anderen PC umziehen seit 1.1.1).*

- **Ein anderes Laufwerk:** **Einstellungen → Speicher → Ändern…**, wähle ein Laufwerk oder einen Ordner, dann:
  - **Instanzen verschieben:** verschiebt deine Instanzen, Server und die gemeinsamen Dateien von Minecraft dorthin.
  - **Ohne Verschieben verwenden:** Neues landet dort; deine bisherigen Instanzen bleiben und kommen zurück, wenn du zurückwechselst.
- **Ein neuer PC:** **Einstellungen → Auf einen anderen PC umziehen** speichert alles in einer `.resincraft`-Datei. Öffne sie auf dem neuen PC mit ResinCraft.

## 22. Fehlerbehebung

- **Das Spiel startet nicht:** Schau dir das Fenster **Minecraft ist abgestürzt** an (Abschnitt 6) oder den Tab **Logs** der Instanz. Fehlen Dateien oder sind sie kaputt? Installiere sie neu (Abschnitt 16).
- **Ein Mod taucht in Entdecken nicht auf:** Vielleicht gibt es ihn für deine Minecraft-Version oder deinen Loader nicht. Probiere eine andere Version, oder ändere die Version der Instanz (Abschnitt 5).
- **Freunde können nicht beitreten:** Prüfe die Adresse im Tab **Console** des Servers und ob der Server **Running** (läuft) anzeigt. Mit ResinCraft Relay: Achte darauf, dass du mit einem Microsoft-Konto angemeldet bist.
- **Der Server stürzt beim Start ab:** ResinCraft schaltet reine Client-Mods von selbst aus und startet ihn neu. In der **Console** steht in einer Zeile, welchen Mod es ausgeschaltet hat. Steht dort, dass Dateien fehlen, hol sie dir aus der Liste oben im Tab **Console**.
- **Kommst du immer noch nicht weiter?** Melde ein Problem auf [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) und häng das Log aus dem Tab **Logs** an.
