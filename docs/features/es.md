# ResinCraft Launcher: funciones

Un launcher de Minecraft moderno y fácil de usar, creado por **Puirkle**, para Windows, macOS y Linux. Sin telemetría y sin cuenta de ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

La versión entre paréntesis es aquella en la que llegó cada función. Las más recientes son 1.4.0 y 1.4.1.

## Jugar

- **Instancias (0.1.0):** instalaciones separadas por versión o modpack, cada una con sus propios mods, mundos, Java, memoria y argumentos de JVM.
- **Cualquier versión de Minecraft (0.1.0):** todas las versiones y snapshots, hasta las alfas clásicas.
- **Cargadores de mods (0.1.0):** Fabric, Quilt, Forge y NeoForge, con selector de versión del cargador. Forge clásico para 1.7.10 a 1.12.2 (1.1.2), Forge hasta Minecraft 1.2.5 y LiteLoader (1.2.0). También OptiFine (1.0.1) y mods de jar (1.2.0).
- **Cambiar versión y cargador (1.2.7):** cambia la versión de Minecraft o el cargador de mods de una instancia desde su pestaña Ajustes, como en Prism. La etiqueta de versión de la página de la instancia te lleva ahí.
- **Jugar (0.1.0):** un clic. Con **Jugar esta vez como** usas otra cuenta, y con **Jugar también como** abres la misma instancia dos veces con dos cuentas (1.0.5).
- **Unión rápida (1.1.0):** tus servidores recientes, y tus mundos de un jugador (1.2.3); un clic y entras directamente.
- **Mundos (0.1.0):** cámbiale el nombre a un mundo, haz una copia de seguridad o importa un mundo desde un zip. Juega un mundo directamente (1.2.0).
- **Copias automáticas de los mundos (1.4.0):** mientras un juego o un servidor está en marcha, se hace una copia de seguridad de sus mundos cada 30 minutos y se guardan las 3 más recientes de cada mundo (Ajustes → Copia de seguridad automática).
- **Restaurar una copia (1.4.0):** recupera un mundo desde cualquier copia, tuya o automática, o tráelo de vuelta como copia. El mundo que se reemplaza va a la Papelera de reciclaje.
- **Reparador de cierres (1.2.7):** cuando Minecraft se cierra inesperadamente, ResinCraft lee el informe de error y los registros. Te explica con palabras sencillas qué ha fallado y te ofrece un arreglo con un clic. Arreglos: instalar un mod que falta, desactivar un mod que ha provocado el cierre, elegir el Java correcto, dar más memoria al juego.
- **Reinstalar (1.3.0):** vuelve a descargar los archivos del juego de una instancia, o devuelve los archivos de un modpack tal como los tiene el pack. Desde el menú de la instancia o **Ajustes → Reparar**. Los mundos se quedan.
- **Flags de Java optimizados (1.2.7):** los juegos arrancan con los flags G1GC de ResinCraft para que los fotogramas vayan más fluidos (Ajustes → Java, o por instancia).
- **Java (0.1.0):** encuentra los Java instalados, elige el adecuado para cada versión de Minecraft y descarga Eclipse Temurin si ninguno sirve.
- **Uso de RAM (1.2.1):** cuánta memoria usan el launcher, cada juego y cada servidor (también en Inicio, si quieres).
- **Discord Rich Presence (1.2.3):** nombre del pack, versión, tiempo de juego, un jugador o servidor. Botones para tus amigos (1.2.6): **View modpack** (ver modpack) y **Get ResinCraft** (consigue ResinCraft).

## Instancias

- **Organiza:** grupos (0.2.0), tu propio orden (0.3.1, arrastra para ordenar), iconos y banners (0.3.0), y colores (0.1.0).
- **Pestañas (0.1.0):** Contenido, Mundos, Servidores, Capturas, Notas, Archivos (gestor y editor de archivos integrado, 0.2.0), Registros y Ajustes.
- **Copiar de otra instancia (1.2.5):** mundos, shaders, paquetes de recursos y ajustes del juego; los mods y sus ajustes (carpeta config) desde 1.2.6.
- **Duplicar (0.1.0), exportar** (Modrinth `.mrpack` o CurseForge `.zip`, 1.0.5) y **convertirla en tu propio modpack (1.1.0)**.
- **Seleccionar varias (1.1.0):** mueve, borra, duplica o actualiza muchas instancias a la vez.
- **Comandos y entorno (1.2.0):** comandos que se ejecutan antes de iniciar y después de cerrar el juego, y variables de entorno.
- **Interruptores instantáneos (1.2.7):** activar o desactivar un mod es instantáneo; el cambio de nombre se hace en segundo plano.
- **Borrar y crear al instante (1.3.0):** una instancia borrada desaparece en el acto y va a la Papelera de reciclaje en segundo plano. Un modpack que se está instalando aparece en la Biblioteca enseguida, con su progreso en la tarjeta.

## Contenido y Descubrir

- **Sitios (0.1.0):** mods, modpacks, paquetes de recursos y shaders de Modrinth, CurseForge, FTB, ATLauncher, Technic y releases de GitHub (0.3.1), filtrados por la versión y el cargador de la instancia.
- **Páginas de modpacks (1.2.0)** con descripción, galería, versiones, registros de cambios y **los mods que contiene el pack (1.2.5)**.
- **Instalaciones inteligentes (0.3.0):** elige cualquier versión, instala varias cosas a la vez, las dependencias necesarias se añaden solas y se detecta el mismo mod repetido desde dos sitios.
- **Actualizaciones (0.1.0):** busca y aplica actualizaciones (una o todas); actualizaciones de modpacks y cambio de versión (1.1.0). **Actualizar todo** descarga hasta cuatro a la vez (1.4.1).
- **Descargas manuales (1.0.1):** los archivos de CurseForge cuyos autores bloquean a los launchers se recogen de tu carpeta de descargas o de otras **Carpetas vigiladas** (1.0.5, Ajustes → Descargas).
- **Listas (0.4.0):** exporta los mods, paquetes de recursos o shaders como lista, e instala una lista en otra instancia.
- **Encontrar servidores (1.4.0):** Descubrir → **Servidores** muestra los servidores alojados con ResinCraft y los que han añadido los jugadores. Busca y filtra por categoría, región, idioma, modpack, y si admiten Bedrock o perfiles sin conexión; dales me gusta y denúncialos.
- **Una página para cada servidor (1.4.1):** la descripción completa y el MOTD, todos los detalles, su pack de cliente, enlaces y un cuadro para pedir la whitelist.
- **Entrar con un clic (1.4.0):** **Entrar** elige la instancia que encaja, o te pregunta. **Instalar y entrar** crea una instancia con exactamente los mods del servidor y entra.
- **Pedir entrar en la whitelist (1.4.0):** **Pedir entrar** envía una solicitud firmada con tu cuenta de Minecraft. Muestra **Waiting for the host** (esperando al anfitrión), **On the whitelist** (en la whitelist) o **Ask again** (volver a pedir), y ResinCraft te avisa (y hace parpadear la barra de tareas) cuando el anfitrión responde (1.4.1).

## Alojar servidores

- **Tipos de servidor (0.1.0):** Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge y NeoForge; Leaf, Arclight (mods y plugins juntos) y Bedrock Dedicated Server desde 0.3.1.
- **Desde un modpack o una instancia (0.1.0):** crea un servidor desde Modrinth, CurseForge (con el server pack oficial si existe) o FTB, o desde una de tus instancias.
- **¿Sin server pack oficial? (1.2.11):** ResinCraft monta el servidor a partir de la lista de mods del propio pack, te lo dice al crear el servidor y muestra la insignia **Built by ResinCraft** (creado por ResinCraft).
- **Archivos que descargas tú (1.2.9):** los archivos cuyos autores solo permiten descargarlos desde su web aparecen en la pestaña **Console** (consola) del servidor. Antes de cada arranque, ResinCraft los busca en tu carpeta de descargas y en tus instancias del mismo pack, y los pone en su sitio.
- **Memoria según el tamaño del pack (1.2.9):** un servidor nuevo con mods recibe 6 GB a partir de 80 mods y 8 GB a partir de 150 (como mucho la mitad de la RAM de tu PC).
- **Mods solo de cliente, desactivados solo si fallan (1.2.8):** los mods de cliente se quedan en el servidor mientras arranque bien. Si un arranque falla porque un mod intentó usar el cliente del juego, ese mod se desactiva (`.jar.disabled`) y el servidor vuelve a arrancar solo. Si otro mod lo necesita, se queda activado.
- **Gestión (0.1.0):** consola, editor de propiedades, gestor de plugins y mods con actualizaciones (0.3.0), whitelist, ops y baneos, icono del servidor (1.2.5), copias de seguridad.
- **Actualizaciones más rápidas (1.4.1):** **Actualizar todo** descarga hasta cuatro plugins o mods a la vez y los instala juntos, o los deja en cola para el siguiente reinicio si el servidor está en marcha.
- **Pon tu servidor en la lista (1.4.0):** Settings → **Lista de servidores** lo pone en Descubrir → Servidores con una descripción, etiquetas, región e idioma. Los servidores que tienes en otro sitio se pueden añadir con **Enviar un servidor**.
- **Packs de cliente (1.4.0):** publica lo que necesitan los jugadores para entrar. Desde 1.4.1: el modpack del servidor (instalado desde su propio sitio), cada mod adicional desde Modrinth o CurseForge, y tus propios archivos directamente desde tu PC; también paquetes de recursos y shaders; también para servidores de plugins y vanilla.
- **Reiniciar y menú de más opciones (1.2.7):** Reiniciar junto a Stop; un menú con copiar dirección, abrir carpeta, hacer copia de seguridad, ajustes y eliminar.
- **Online now** (conectados ahora, 1.2.7): los jugadores de tu servidor, comprobados en directo cada pocos segundos.
- **Java flags** (flags de Java, 0.2.0): los flags optimizados de ResinCraft o los del propio modpack (`user_jvm_args.txt`, scripts de inicio, 1.2.7).
- **Anti-xray (1.3.0):** los servidores Paper, Purpur, Folia y Leaf pueden ocultar los minerales a los mods y paquetes de texturas de rayos X (Settings → Anti-xray).
- **Anti-cheat (1.4.0):** un solo interruptor instala el anticheat Grim en los servidores de plugins.
- **Regenerar mundo (1.4.0):** vuelve a empezar un servidor con un mundo nuevo; el antiguo va a la Papelera de reciclaje.
- **Reinstalar (1.3.0):** vuelve a instalar el software de un servidor, o devuelve los archivos de su modpack. El mundo y los ajustes se quedan.
- **Borrar y crear al instante (1.3.0):** un servidor borrado desaparece en el acto; un servidor que se está creando aparece en la lista enseguida, con su progreso.
- **Control remoto (1.3.0):** maneja tus servidores desde cualquier navegador o desde el móvil con la consola web en `console.rscraft.resincore.tech`, con la misma cuenta de Minecraft, mientras ResinCraft está abierto o en la bandeja del sistema (Ajustes → Control remoto). Puede hacer todo lo que hace el gestor de servidores. El gestor de archivos, los ajustes de Java y los plugins desde cualquier enlace necesitan un segundo interruptor, **Permitir control total**. Un botón **Consola web** en la página Servers (1.3.1).
- **Tus otros PC (1.4.1):** el menú de PC de la pestaña Servers abre los servidores de tus otros PC que tienen el control remoto activado: iniciar, detener, gestionar jugadores, plugins y ajustes, y crear o importar servidores allí.
- **Deja que tus amigos ayuden (1.4.1):** Ajustes → Control remoto → **People who can use your servers** (personas que pueden usar tus servidores): da a una cuenta de Minecraft acceso de **Viewer** (observador), **Operator** (operador) o **Manager** (gestor), a todos tus servidores o a los que elijas. Los archivos, los ajustes de Java y crear o borrar servidores siguen siendo cosa tuya.
- **Compartir con amigos:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`, 0.3.0)
  - playit.gg (0.2.0)
  - bore (0.2.0)
  - tu router (UPnP, 0.2.0)
  - tu propio dominio, con DNS de Cloudflare automático (0.2.0), verificado con un registro TXT (**Verificación pendiente**, **Verificado**, 1.3.1)
  - **Automatic** (automático) para los servidores nuevos (1.3.2): primero tu router, luego ResinCraft Relay, luego playit.gg
- **Bedrock players can join** (los jugadores de Bedrock pueden entrar, 0.3.1): los jugadores de Bedrock también entran a servidores Java (Geyser + Floodgate).
- **Whitelist requests** (solicitudes de whitelist, 1.2.7): los jugadores piden entrar y tú los aceptas o rechazas en la pestaña Players.
  - **Web page** (página web): en la propia dirección de tu servidor o en tu dominio verificado (1.3.2, con ResinCraft Relay).
  - **Your Discord bot** (tu propio bot de Discord): los jugadores escriben `/whitelist`.
  - **La lista de servidores (1.4.0):** los jugadores con ResinCraft lo piden desde Descubrir → Servidores.
  - **Tu respuesta:** Accept (aceptar) los añade a la whitelist; el bot se lo dice por mensaje directo.
  - **Protección:** los jugadores baneados se rechazan; **Reject and block** (rechazar y bloquear) evita que alguien vuelva a pedirlo, con una lista **Blocked** (bloqueados) y **Unblock** (desbloquear) para cada jugador (1.4.1).

## Bedrock Edition (Windows)

- **Página de Bedrock (0.3.1):** instala (1.0.0), actualiza y repara (1.2.0), abre y desinstala (1.2.3) Minecraft para Windows y Minecraft Preview (antes se hace una copia de seguridad de los mundos).

## Cuentas

- **Inicio de sesión con Microsoft (0.1.0)**, con los tokens guardados cifrados. **Perfiles sin conexión (0.1.0)** para un jugador y LAN. **Perfiles de Bedrock (1.0.0)** para cuentas de Microsoft sin Java Edition.
- **Aspectos, capas y cambio de nombre (0.1.0).** Una biblioteca de aspectos, aspectos a partir de un nombre de jugador y búsqueda en línea (1.0.3); un visor de aspectos en 3D (1.1.0).

## Venir desde otro launcher

- **Importar (1.0.5):** instancias de Prism Launcher, MultiMC, la Modrinth App, la app de CurseForge, ATLauncher, GDLauncher y el Minecraft Launcher oficial, con sus mods, mundos, packs y ajustes. También se pueden importar servidores.
- **Pasar a otro PC (1.1.1):** Ajustes → Pasar a otro PC guarda todo en un solo archivo `.resincraft`; ábrelo en el PC nuevo.

## La app

- **Windows (0.1.0), macOS** (Apple Silicon e Intel) **y Linux** (x64 y ARM64: AppImage, `.deb`, 1.2.0; `.tar.gz`, 1.2.4).
- **Instaladores para Windows:** el `.exe` (0.1.0), un `.msi` para instalaciones gestionadas (1.2.10) y **ResinCraft Setup (1.3.0)**: un instalador pequeño que siempre instala la versión más reciente, en un enlace que nunca cambia.
- **Actualizaciones (0.3.0):** se buscan automáticamente.
  - Un botón **Actualizar** en la barra de título (1.2.7).
  - **Cerrarlos y actualizar**, cuando hay juegos o servidores en marcha (1.2.7).
  - **Actualizar en silencio** en segundo plano (1.2.7).
- **Seguir en la bandeja del sistema (1.2.7):** al cerrar la ventana, los servidores, el bot de Discord, el control remoto y las descargas siguen funcionando.
- **Reinstalar ResinCraft (1.3.0):** Ajustes → Actualizaciones vuelve a instalar la app sobre sí misma para repararla. Las instancias, cuentas y ajustes se quedan.
- **Temas:** Oscuro, Claro y Sistema (0.1.0); Obsidiana, Pizarra profunda, Cerezo, Nether y El End (1.3.0).
  - **Crear tema (1.3.0):** tu propio tema, con **Ajuste fino** para cualquier color (selectores de color) y CSS para partes de la app (1.3.2).
  - **Compartir temas (1.3.2):** **Exportar** un archivo `.rctheme`, con su imagen y su CSS; **Importar tema**, o haz doble clic en el archivo (1.4.0).
  - **CSS propio (1.3.0):** tu propio CSS sobre cualquier tema, partiendo de los colores actuales, con una guía integrada de los colores, las partes de la app y ejemplos listos para usar.
  - Y una sorpresa escondida (1.3.0). Prueba el código Konami.
- **16 idiomas (0.1.0):** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Gestor de almacenamiento (1.1.0):** guarda las instancias en otra unidad. **Mover instancias** allí, o **Usar sin mover** (1.4.1).
- **Restablecer ajustes** y **Desinstalar ResinCraft (1.4.0)** en Ajustes.
