# ResinCraft Launcher: funciones

Un launcher de Minecraft moderno y fácil de usar, creado por **Puirkle**, para Windows, macOS y Linux. Sin telemetría y sin cuenta de ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Lo marcado con **(nuevo)** es nuevo en la última versión.

## Jugar

- **Instancias:** instalaciones separadas por versión o modpack, cada una con sus propios mods, mundos, Java, memoria y argumentos de JVM.
- **Cualquier versión de Minecraft:** todas las versiones y snapshots, hasta las alfas clásicas.
- **Cargadores de mods:** Fabric, Quilt, Forge (hasta Minecraft 1.2.5), NeoForge y LiteLoader, con selector de versión del cargador. También OptiFine y mods de jar.
- **Cambiar versión y cargador:** cambia la versión de Minecraft o el cargador de mods de una instancia desde su pestaña Ajustes, como en Prism. La etiqueta de versión de la página de la instancia te lleva ahí.
- **Jugar:** un clic. Con **Jugar esta vez como** usas otra cuenta, y con **Jugar también como** abres la misma instancia dos veces con dos cuentas.
- **Unión rápida:** tus servidores y mundos de un jugador recientes; un clic y entras directamente.
- **Mundos:** juega un mundo directamente, cámbiale el nombre, haz una copia de seguridad o importa un mundo desde un zip.
- **Reparador de cierres:** cuando Minecraft se cierra inesperadamente, ResinCraft lee el informe de error y los registros. Te explica con palabras sencillas qué ha fallado y te ofrece un arreglo con un clic. Arreglos: instalar un mod que falta, desactivar un mod que ha provocado el cierre, elegir el Java correcto, dar más memoria al juego.
- **Reinstalar (nuevo):** vuelve a descargar los archivos del juego de una instancia, o devuelve los archivos de un modpack tal como los tiene el pack. Desde el menú de la instancia o **Ajustes → Reparar**. Los mundos se quedan.
- **Flags de Java optimizados:** los juegos arrancan con los flags G1GC de ResinCraft para que los fotogramas vayan más fluidos (Ajustes → Java, o por instancia).
- **Java:** encuentra los Java instalados, elige el adecuado para cada versión de Minecraft y descarga Eclipse Temurin si ninguno sirve.
- **Uso de RAM:** cuánta memoria usan el launcher, cada juego y cada servidor (también en Inicio, si quieres).
- **Discord Rich Presence:** nombre del pack, versión, tiempo de juego, un jugador o servidor. Botones para tus amigos: **View modpack** (ver modpack) y **Get ResinCraft** (consigue ResinCraft).

## Instancias

- **Organiza:** grupos, tu propio orden (arrastra para ordenar), iconos, banners y colores.
- **Pestañas:** Contenido, Mundos, Servidores, Capturas, Notas, Archivos (gestor y editor de archivos integrado), Registros y Ajustes.
- **Copiar de otra instancia:** mundos, mods y sus ajustes (carpeta config), shaders, paquetes de recursos y ajustes del juego.
- **Duplicar, exportar** (Modrinth `.mrpack` o CurseForge `.zip`) y **convertirla en tu propio modpack**.
- **Comandos y entorno:** comandos que se ejecutan antes de iniciar y después de cerrar el juego, y variables de entorno.
- **Interruptores instantáneos:** activar o desactivar un mod es instantáneo; el cambio de nombre se hace en segundo plano.
- **Borrar y crear al instante (nuevo):** una instancia borrada desaparece en el acto y va a la Papelera de reciclaje en segundo plano. Un modpack que se está instalando aparece en la Biblioteca enseguida, con su progreso en la tarjeta.

## Contenido y Descubrir

- **Sitios:** mods, modpacks, paquetes de recursos y shaders de Modrinth, CurseForge, FTB, ATLauncher, Technic y releases de GitHub, filtrados por la versión y el cargador de la instancia.
- **Páginas de modpacks** con descripción, galería, versiones, registros de cambios y **los mods que contiene el pack**.
- **Instalaciones inteligentes:** elige cualquier versión, instala varias cosas a la vez, las dependencias necesarias se añaden solas y se detecta el mismo mod repetido desde dos sitios.
- **Actualizaciones:** busca y aplica actualizaciones (una o todas); actualizaciones de modpacks y cambio de versión.
- **Descargas manuales:** los archivos de CurseForge cuyos autores bloquean a los launchers se recogen de tu carpeta de descargas o de otras **Carpetas vigiladas** (Ajustes → Descargas).
- **Listas:** exporta los mods, paquetes de recursos o shaders como lista, e instala una lista en otra instancia.

## Alojar servidores

- **Tipos de servidor:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mods y plugins juntos) y Bedrock Dedicated Server.
- **Desde un modpack o una instancia:** crea un servidor desde Modrinth, CurseForge (con el server pack oficial si existe) o FTB, o desde una de tus instancias.
- **¿Sin server pack oficial? (nuevo):** ResinCraft monta el servidor a partir de la lista de mods del propio pack, te lo dice al crear el servidor y muestra la insignia **Built by ResinCraft** (creado por ResinCraft).
- **Archivos que descargas tú (nuevo):** los archivos cuyos autores solo permiten descargarlos desde su web aparecen en la pestaña **Console** (consola) del servidor. Antes de cada arranque, ResinCraft los busca en tu carpeta de descargas y en tus instancias del mismo pack, y los pone en su sitio.
- **Memoria según el tamaño del pack (nuevo):** un servidor nuevo con mods recibe 6 GB a partir de 80 mods y 8 GB a partir de 150 (como mucho la mitad de la RAM de tu PC).
- **Mods solo de cliente, desactivados solo si fallan:** los mods de cliente se quedan en el servidor mientras arranque bien. Si un arranque falla porque un mod intentó usar el cliente del juego, ese mod se desactiva (`.jar.disabled`) y el servidor vuelve a arrancar solo. Si otro mod lo necesita, se queda activado.
- **Gestión:** consola, editor de propiedades, gestor de plugins y mods con actualizaciones, whitelist, ops y baneos, icono del servidor, copias de seguridad.
- **Reiniciar y menú de más opciones:** Reiniciar junto a Stop; un menú con copiar dirección, abrir carpeta, hacer copia de seguridad, ajustes y eliminar.
- **Online now** (conectados ahora): los jugadores de tu servidor, comprobados en directo cada pocos segundos.
- **Java flags** (flags de Java): los flags optimizados de ResinCraft o los del propio modpack (`user_jvm_args.txt`, scripts de inicio).
- **Anti-xray (nuevo):** los servidores Paper, Purpur, Folia y Leaf pueden ocultar los minerales a los mods y paquetes de texturas de rayos X (Settings → Anti-xray).
- **Reinstalar (nuevo):** vuelve a instalar el software de un servidor, o devuelve los archivos de su modpack. El mundo y los ajustes se quedan.
- **Borrar y crear al instante (nuevo):** un servidor borrado desaparece en el acto; un servidor que se está creando aparece en la lista enseguida, con su progreso.
- **Control remoto (nuevo):** maneja tus servidores desde cualquier navegador o desde el móvil con la consola web en `console.rscraft.resincore.tech`, con la misma cuenta de Minecraft, mientras ResinCraft está abierto o en la bandeja del sistema (Ajustes → Control remoto). Puede iniciar, detener, vigilar y crear servidores, pero nunca llega a tus archivos ni a otros ajustes.
- **Compartir con amigos:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - tu router (UPnP)
  - tu propio dominio, con DNS de Cloudflare automático
- **Bedrock players can join** (los jugadores de Bedrock pueden entrar): los jugadores de Bedrock también entran a servidores Java (Geyser + Floodgate).
- **Whitelist requests** (solicitudes de whitelist): los jugadores piden entrar y tú los aceptas o rechazas en la pestaña Players.
  - **Web page** (página web): `https://rscraft.resincore.tech/join/<name>` (con ResinCraft Relay).
  - **Your Discord bot** (tu propio bot de Discord): los jugadores escriben `/whitelist`.
  - **Tu respuesta:** Accept (aceptar) los añade a la whitelist; el bot se lo dice por mensaje directo.
  - **Protección:** los jugadores baneados se rechazan; **Reject and block** (rechazar y bloquear) evita que alguien vuelva a pedirlo.

## Bedrock Edition (Windows)

- Instala, abre y desinstala Minecraft para Windows y Minecraft Preview (antes se hace una copia de seguridad de los mundos).

## Cuentas

- **Inicio de sesión con Microsoft**, con los tokens guardados cifrados. **Perfiles sin conexión** para un jugador y LAN.
- **Aspectos, capas y cambio de nombre.**

## Venir desde otro launcher

- Importa instancias de Prism Launcher, MultiMC, la Modrinth App, la app de CurseForge, ATLauncher, GDLauncher y el Minecraft Launcher oficial, con sus mods, mundos, packs y ajustes. También se pueden importar servidores.

## La app

- **Windows, macOS** (Apple Silicon e Intel) **y Linux** (x64 y ARM64: AppImage, `.deb`, `.tar.gz`).
- **Instaladores para Windows:** el `.exe`, un `.msi` para instalaciones gestionadas y **ResinCraft Setup (nuevo)**: un instalador pequeño que siempre instala la versión más reciente, en un enlace que nunca cambia.
- **Actualizaciones:** se buscan automáticamente.
  - Un botón **Actualizar** en la barra de título.
  - **Cerrarlos y actualizar**, cuando hay juegos o servidores en marcha.
  - **Actualizar en silencio** en segundo plano.
- **Seguir en la bandeja del sistema:** al cerrar la ventana, los servidores, el bot de Discord, el control remoto y las descargas siguen funcionando.
- **Reinstalar ResinCraft (nuevo):** Ajustes → Actualizaciones vuelve a instalar la app sobre sí misma para repararla. Las instancias, cuentas y ajustes se quedan.
- **Temas (nuevo):** Oscuro, Claro, Obsidiana, Pizarra profunda, Cerezo, Nether, El End, Halloween y Navidad, o Sistema.
  - Halloween se activa solo del 16 al 31 de octubre, y Navidad del 10 al 25 de diciembre. Si eliges otro tema, se queda el tuyo; o desactiva los temas festivos.
  - **CSS propio:** tu propio CSS sobre cualquier tema, partiendo de los colores actuales.
  - Y una sorpresa escondida. Prueba el código Konami.
- **16 idiomas:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Almacenamiento** (mueve los datos a otra unidad), **copia de seguridad y restauración** de los ajustes.
