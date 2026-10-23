# ResinCraft Launcher: funzionalità

Un launcher di Minecraft moderno e semplice da usare, creato da **Puirkle**, per Windows, macOS e Linux. Nessuna telemetria e nessun account ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Le voci segnate con **(1.2.7)** sono nuove nella versione 1.2.7.

## Giocare

- **Istanze:** installazioni separate per versione o modpack, ognuna con mod, mondi, Java, memoria e argomenti JVM propri.
- **Qualsiasi versione di Minecraft:** tutte le release e gli snapshot, fino alle vecchie alpha classic.
- **Mod loader:** Fabric, Quilt, Forge (fino a Minecraft 1.2.5), NeoForge e LiteLoader, con la scelta della versione del loader. Anche OptiFine e le jar mod.
- **Cambia versione e loader (1.2.7):** cambia la versione di Minecraft o il mod loader di un’istanza dalla sua scheda Impostazioni, come in Prism. Il chip della versione nella pagina dell’istanza ti porta lì.
- **Gioca:** un clic. Con **Gioca questa volta come** usi un altro account, con **Gioca anche come** avvii la stessa istanza due volte con due account.
- **Accesso rapido:** i tuoi server e mondi in giocatore singolo più recenti; un clic e sei subito dentro.
- **Mondi:** gioca direttamente in un mondo, rinominalo, fanne un backup, importa un mondo da uno zip.
- **Risolvi crash (1.2.7):** quando Minecraft va in crash, ResinCraft legge il crash report e i log. Ti spiega con parole semplici cosa è andato storto e ti propone una soluzione con un clic. Soluzioni: installare una mod mancante, disattivare una mod che ha mandato in crash il gioco, scegliere il Java giusto, dare più memoria al gioco.
- **Java:** trova i Java installati, sceglie quello giusto per ogni versione di Minecraft e scarica Eclipse Temurin se nessuno va bene.
- **Uso della RAM:** quanta memoria usano il launcher, ogni gioco e ogni server (anche nella Home, se vuoi).
- **Discord Rich Presence:** nome del pack, versione, tempo di gioco, giocatore singolo o server. Pulsanti per gli amici: **View modpack** (vedi il modpack) e **Get ResinCraft** (scarica ResinCraft).

## Istanze

- **Organizza:** gruppi, il tuo ordine (trascina per riordinare), icone, banner e colori.
- **Schede:** Contenuti, Mondi, Server, Screenshot, Note, File (file manager ed editor integrati), Log e Impostazioni.
- **Copia da un’altra istanza:** mondi, mod e le loro impostazioni (cartella config), shader pack, pacchetti di risorse e impostazioni di gioco.
- **Duplica, esporta** (Modrinth `.mrpack` o CurseForge `.zip`) e **trasformala nel tuo modpack**.
- **Comandi e ambiente:** comandi da eseguire prima dell’avvio e dopo la chiusura del gioco, e variabili d’ambiente.
- **Interruttori istantanei (1.2.7):** attivare o disattivare una mod è istantaneo; la rinomina avviene in background.

## Contenuti e Scopri

- **Siti:** mod, modpack, pacchetti di risorse e shader da Modrinth, CurseForge, FTB, ATLauncher, Technic e dalle release di GitHub, filtrati per versione e loader dell’istanza.
- **Pagine dei modpack** con descrizione, galleria, versioni, changelog e **le mod contenute nel pack**.
- **Installazioni intelligenti:** scegli qualsiasi versione, installa più cose insieme, le dipendenze richieste vengono aggiunte da sole e la stessa mod presa da due siti viene riconosciuta.
- **Aggiornamenti:** cerca e applica gli aggiornamenti (uno o tutti); aggiornamenti dei modpack e cambio di versione.
- **Download manuali:** i file di CurseForge i cui autori bloccano i launcher vengono presi dalla tua cartella Download o da altre **Cartelle controllate** (Impostazioni → Download).
- **Elenchi:** esporta mod, pacchetti di risorse o shader come elenco e installa un elenco in un’altra istanza.

## Ospitare server

- **Tipi di server:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mod e plugin insieme) e Bedrock Dedicated Server.
- **Da un modpack o da un’istanza:** crea un server da Modrinth, CurseForge (con il server pack ufficiale, se c’è) o FTB, oppure da una delle tue istanze.
- **Mod solo client tenute spente:** le mod che funzionano solo sul client del gioco vengono trovate e disattivate in automatico, a ogni avvio del server. Una mod che serve a un’altra mod resta attiva. Se una manda comunque in crash il server, viene disattivata e il server si riavvia da solo.
- **Gestione:** console, editor delle proprietà, gestione di plugin e mod con aggiornamenti, whitelist, op e ban, icona del server, backup.
- **Riavvia e menu altro (1.2.7):** Riavvia accanto a Stop; un menu con copia indirizzo, apri cartella, backup, impostazioni ed elimina.
- **Online now** (online ora): i giocatori sul tuo server, controllati dal vivo ogni pochi secondi **(1.2.7)**.
- **Java flags (1.2.7)** (flag Java): i flag ottimizzati di ResinCraft o quelli del modpack (`user_jvm_args.txt`, script di avvio).
- **Condividi con gli amici:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - il tuo router (UPnP)
  - il tuo dominio, con DNS Cloudflare automatico
- **Bedrock players can join** (i giocatori Bedrock possono entrare): anche chi gioca a Bedrock può entrare nei server Java (Geyser + Floodgate).
- **Whitelist requests (1.2.7)** (richieste di whitelist): i giocatori chiedono di entrare e tu li accetti o li rifiuti nella scheda Players.
  - **Web page** (pagina web): `https://rscraft.resincore.tech/join/<name>` (con ResinCraft Relay).
  - **Your Discord bot** (il tuo bot Discord): i giocatori scrivono `/whitelist`.
  - **La tua risposta:** Accept (accetta) li mette in whitelist; il bot glielo dice con un messaggio diretto.
  - **Protezione:** i giocatori bannati vengono rifiutati; **Reject and block** (rifiuta e blocca) impedisce a qualcuno di chiedere di nuovo.

## Bedrock Edition (Windows)

- Installa, avvia e disinstalla Minecraft per Windows e Minecraft Preview (prima viene fatto un backup dei mondi).

## Account

- **Accesso con Microsoft**, con i token salvati cifrati. **Profili offline** per giocatore singolo e LAN.
- **Skin, mantelli e cambio nome.**

## Trasferirsi da un altro launcher

- Importa istanze da Prism Launcher, MultiMC, Modrinth App, app di CurseForge, ATLauncher, GDLauncher e dal Minecraft Launcher ufficiale, con le loro mod, mondi, pack e impostazioni. Si possono importare anche i server.

## L’app

- **Windows, macOS** (Apple Silicon e Intel) **e Linux** (x64 e ARM64: AppImage, `.deb`, `.tar.gz`).
- **Aggiornamenti:** controllati in automatico.
  - **(1.2.7)** Un pulsante **Aggiorna** nella barra del titolo.
  - **(1.2.7)** **Chiudili e aggiorna**, quando ci sono giochi o server in esecuzione.
  - **(1.2.7)** **Aggiorna in silenzio** in background.
- **Resta nell’area di notifica (1.2.7):** chiudendo la finestra, server, bot Discord e download continuano a funzionare.
- **16 lingue:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Archiviazione** (sposta i dati su un altro disco), **backup e ripristino** delle impostazioni, temi **chiaro e scuro**.
