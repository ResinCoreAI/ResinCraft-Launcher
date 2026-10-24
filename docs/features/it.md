# ResinCraft Launcher: funzionalità

Un launcher di Minecraft moderno e semplice da usare, creato da **Puirkle**, per Windows, macOS e Linux. Nessuna telemetria e nessun account ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Le voci segnate con **(novità)** sono nuove nell’ultima versione.

## Giocare

- **Istanze:** installazioni separate per versione o modpack, ognuna con mod, mondi, Java, memoria e argomenti JVM propri.
- **Qualsiasi versione di Minecraft:** tutte le release e gli snapshot, fino alle vecchie alpha classic.
- **Mod loader:** Fabric, Quilt, Forge (fino a Minecraft 1.2.5), NeoForge e LiteLoader, con la scelta della versione del loader. Anche OptiFine e le jar mod.
- **Cambia versione e loader:** cambia la versione di Minecraft o il mod loader di un’istanza dalla sua scheda Impostazioni, come in Prism. Il chip della versione nella pagina dell’istanza ti porta lì.
- **Gioca:** un clic. Con **Gioca questa volta come** usi un altro account, con **Gioca anche come** avvii la stessa istanza due volte con due account.
- **Accesso rapido:** i tuoi server e mondi in giocatore singolo più recenti; un clic e sei subito dentro.
- **Mondi:** gioca direttamente in un mondo, rinominalo, fanne un backup, importa un mondo da uno zip.
- **Risolvi crash:** quando Minecraft va in crash, ResinCraft legge il crash report e i log. Ti spiega con parole semplici cosa è andato storto e ti propone una soluzione con un clic. Soluzioni: installare una mod mancante, disattivare una mod che ha mandato in crash il gioco, scegliere il Java giusto, dare più memoria al gioco.
- **Reinstalla (novità):** scarica di nuovo i file del gioco di un’istanza, o rimetti i file di un modpack come li ha il pack. Dal menu dell’istanza o da **Impostazioni → Ripara**. I mondi restano.
- **Flag Java ottimizzati:** i giochi partono con i flag G1GC di ResinCraft per frame più fluidi (Impostazioni → Java, o per singola istanza).
- **Java:** trova i Java installati, sceglie quello giusto per ogni versione di Minecraft e scarica Eclipse Temurin se nessuno va bene.
- **Uso della RAM:** quanta memoria usano il launcher, ogni gioco e ogni server (anche nella Home, se vuoi).
- **Discord Rich Presence:** nome del pack, versione, tempo di gioco, giocatore singolo o server. Pulsanti per gli amici: **View modpack** (vedi il modpack) e **Get ResinCraft** (scarica ResinCraft).

## Istanze

- **Organizza:** gruppi, il tuo ordine (trascina per riordinare), icone, banner e colori.
- **Schede:** Contenuti, Mondi, Server, Screenshot, Note, File (file manager ed editor integrati), Log e Impostazioni.
- **Copia da un’altra istanza:** mondi, mod e le loro impostazioni (cartella config), shader pack, pacchetti di risorse e impostazioni di gioco.
- **Duplica, esporta** (Modrinth `.mrpack` o CurseForge `.zip`) e **trasformala nel tuo modpack**.
- **Comandi e ambiente:** comandi da eseguire prima dell’avvio e dopo la chiusura del gioco, e variabili d’ambiente.
- **Interruttori istantanei:** attivare o disattivare una mod è istantaneo; la rinomina avviene in background.
- **Eliminazione e creazione istantanee (novità):** un’istanza eliminata sparisce subito e finisce nel Cestino in background. Un modpack in fase di installazione compare subito nella Libreria, con l’avanzamento sulla sua scheda.

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
- **Nessun server pack ufficiale? (novità):** ResinCraft costruisce il server dall’elenco di mod del pack, te lo dice quando il server viene creato e mostra il badge **Built by ResinCraft** (creato da ResinCraft).
- **File da scaricare tu (novità):** i file i cui autori permettono il download solo dal loro sito sono elencati nella scheda **Console** del server. Prima di ogni avvio, ResinCraft li cerca nella tua cartella Download e nelle tue istanze dello stesso pack, e li mette al loro posto.
- **Memoria in base alla dimensione del pack (novità):** un nuovo server con mod riceve 6 GB da 80 mod in su e 8 GB da 150 in su (al massimo metà della RAM del tuo PC).
- **Mod solo client disattivate solo se vanno in crash:** le mod client restano sul server finché si avvia senza problemi. Se un avvio va in crash perché una mod ha cercato il client del gioco, quella mod viene disattivata (`.jar.disabled`) e il server si riavvia da solo. Una mod che serve a un’altra mod resta attiva.
- **Gestione:** console, editor delle proprietà, gestione di plugin e mod con aggiornamenti, whitelist, op e ban, icona del server, backup.
- **Riavvia e menu altro:** Riavvia accanto a Stop; un menu con copia indirizzo, apri cartella, backup, impostazioni ed elimina.
- **Online now** (online ora): i giocatori sul tuo server, controllati dal vivo ogni pochi secondi.
- **Java flags** (flag Java): i flag ottimizzati di ResinCraft o quelli del modpack (`user_jvm_args.txt`, script di avvio).
- **Anti-xray (novità):** i server Paper, Purpur, Folia e Leaf possono nascondere i minerali alle mod e ai pacchetti di texture x-ray (Settings → Anti-xray).
- **Reinstalla (novità):** reinstalla il software di un server, o rimetti i file del suo modpack. Il mondo e le impostazioni restano.
- **Eliminazione e creazione istantanee (novità):** un server eliminato sparisce subito; un server in fase di creazione compare subito nell’elenco, con il suo avanzamento.
- **Controllo remoto (novità):** gestisci i tuoi server da qualsiasi browser o dal telefono con la console web su `console.rscraft.resincore.tech`, con lo stesso account Minecraft, mentre ResinCraft è aperto o nell’area di notifica (Impostazioni → Controllo remoto). Può avviare, fermare, seguire e creare server, ma non arriva mai ai tuoi file né ad altre impostazioni.
- **Condividi con gli amici:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - il tuo router (UPnP)
  - il tuo dominio, con DNS Cloudflare automatico
- **Bedrock players can join** (i giocatori Bedrock possono entrare): anche chi gioca a Bedrock può entrare nei server Java (Geyser + Floodgate).
- **Whitelist requests** (richieste di whitelist): i giocatori chiedono di entrare e tu li accetti o li rifiuti nella scheda Players.
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
- **Installer per Windows:** il `.exe`, un `.msi` per le installazioni gestite e **ResinCraft Setup (novità)**: un piccolo installer che installa sempre la versione più recente, da un link che non cambia mai.
- **Aggiornamenti:** controllati in automatico.
  - Un pulsante **Aggiorna** nella barra del titolo.
  - **Chiudili e aggiorna**, quando ci sono giochi o server in esecuzione.
  - **Aggiorna in silenzio** in background.
- **Resta nell’area di notifica:** chiudendo la finestra, server, bot Discord, controllo remoto e download continuano a funzionare.
- **Reinstalla ResinCraft (novità):** Impostazioni → Aggiornamenti reinstalla l’app sopra sé stessa, per ripararla. Istanze, account e impostazioni restano.
- **Temi (novità):** Scuro, Chiaro, Ossidiana, Ardesia abissale, Ciliegio, Nether, L’End, Halloween e Natale, oppure Sistema.
  - Halloween si attiva da solo dal 16 al 31 ottobre, Natale dal 10 al 25 dicembre. Scegli un altro tema e resta il tuo; oppure disattiva i temi delle feste.
  - **CSS personalizzato:** il tuo CSS sopra qualsiasi tema, partendo dai colori attuali.
  - E una sorpresa nascosta. Prova il codice Konami.
- **16 lingue:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Archiviazione** (sposta i dati su un altro disco), **backup e ripristino** delle impostazioni.
