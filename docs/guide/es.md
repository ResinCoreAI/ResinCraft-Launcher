# ResinCraft Launcher: guía de uso

Cómo hacer lo más habitual en ResinCraft. Los nombres en **negrita** son lo que verás en pantalla. Algunas partes de la página de servidores todavía no están traducidas y la app las muestra en inglés; por eso aquí también aparecen en inglés (con la traducción entre paréntesis). Cada sección indica en qué versión llegó.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Contenido

1. [Instalar e iniciar sesión](#1-instalar-e-iniciar-sesión)
2. [Crea tu primera instancia](#2-crea-tu-primera-instancia)
3. [Añadir mods, paquetes de recursos y shaders](#3-añadir-mods-paquetes-de-recursos-y-shaders)
4. [Instalar un modpack](#4-instalar-un-modpack)
5. [Cambiar la versión de Minecraft o el cargador de mods](#5-cambiar-la-versión-de-minecraft-o-el-cargador-de-mods)
6. [Cuando Minecraft se cierra inesperadamente](#6-cuando-minecraft-se-cierra-inesperadamente)
7. [Copiar cosas de otra instancia](#7-copiar-cosas-de-otra-instancia)
8. [Alojar un servidor](#8-alojar-un-servidor)
9. [Deja que tus amigos entren a tu servidor](#9-deja-que-tus-amigos-entren-a-tu-servidor)
10. [Solicitudes de whitelist (página web y bot de Discord)](#10-solicitudes-de-whitelist-página-web-y-bot-de-discord)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Venir desde otro launcher](#12-venir-desde-otro-launcher)
13. [Actualizaciones y la bandeja del sistema](#13-actualizaciones-y-la-bandeja-del-sistema)
14. [Temas y CSS propio](#14-temas-y-css-propio)
15. [Control remoto (consola web)](#15-control-remoto-consola-web)
16. [Reinstalar y reparar](#16-reinstalar-y-reparar)
17. [Encontrar servidores y entrar](#17-encontrar-servidores-y-entrar)
18. [Pon tu servidor en la lista y comparte sus mods](#18-pon-tu-servidor-en-la-lista-y-comparte-sus-mods)
19. [Tus otros PC y los amigos que ayudan](#19-tus-otros-pc-y-los-amigos-que-ayudan)
20. [Copias de seguridad de los mundos](#20-copias-de-seguridad-de-los-mundos)
21. [Almacenamiento y pasar a otro PC](#21-almacenamiento-y-pasar-a-otro-pc)
22. [Solución de problemas](#22-solución-de-problemas)

## 1. Instalar e iniciar sesión

*Desde la versión 0.1.0.*

1. Descarga ResinCraft para tu sistema desde la [página de versiones](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** [`ResinCraft-Setup.exe`](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/download/setup/ResinCraft-Setup.exe) (siempre instala la versión más reciente), o `…_amd64_windows.exe` / `…_amd64_windows.msi`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` o `.tar.gz`
2. Instálalo y ábrelo. Puede que Windows diga que el editor no está verificado; en macOS, la primera vez haz clic derecho en la app y elige **Abrir**.
3. Abre **Cuentas** → **Iniciar sesión con Microsoft** e inicia sesión en la propia página de Microsoft. Tu contraseña nunca pasa por ResinCraft.
   - ¿No tienes cuenta de Microsoft? Añade un **Perfil sin conexión** para jugar solo y en LAN.

## 2. Crea tu primera instancia

*Desde la versión 0.1.0.*

Una *instancia* es una instalación de Minecraft aparte, con sus propios mods, mundos y ajustes.

1. Abre **Biblioteca** → **Nueva instancia**.
2. Elige una **Versión de Minecraft** y un **Cargador de mods**:
   - **Vanilla:** sin mods.
   - **Fabric** o **Quilt:** mods ligeros y de rendimiento.
   - **Forge** o **NeoForge:** mods grandes de contenido.
3. Ponle un nombre y pulsa **Crear instancia**.
4. Pulsa **Jugar**. La primera vez se descargan el juego y el Java adecuado; las siguientes, arranca rápido.

Consejo: **Unión rápida**, a la izquierda, muestra tus servidores y mundos recientes; un clic y entras directamente.

## 3. Añadir mods, paquetes de recursos y shaders

*Desde la versión 0.1.0.*

1. Abre tu instancia → pestaña **Contenido** → **Añadir mods**. O abre **Descubrir** en la barra de la izquierda.
2. Busca, o explora Modrinth, CurseForge, FTB y los demás sitios. Solo se muestran las versiones que encajan con tu instancia.
3. Pulsa **Instalar**. Los mods necesarios (dependencias) se añaden solos.
4. De vuelta en **Contenido**:
   - usa el interruptor junto a un mod para activarlo o desactivarlo;
   - pulsa **Buscar actualizaciones** para actualizar mods (uno o todos). **Actualizar todo** descarga hasta cuatro a la vez.

Algunos autores de CurseForge no permiten que los launchers descarguen sus archivos. En ese caso, ResinCraft los muestra en **Descargar todo**: descárgalos con tu navegador y ResinCraft los encontrará solo en tu carpeta de descargas. ¿Tu navegador guarda en otro sitio? Añade esa carpeta en **Ajustes → Descargas → Carpetas vigiladas**.

## 4. Instalar un modpack

*Desde la versión 0.1.0.*

1. Abre **Descubrir** → **Modpacks** (o **Biblioteca** → **Nueva instancia** → **Descargar un modpack**).
2. Abre un pack para ver su descripción, sus versiones y los **Mods** que contiene.
3. Pulsa **Instalar pack**. La instancia nueva aparece enseguida en la **Biblioteca**, con el progreso de la instalación en su tarjeta; **Jugar** funciona cuando termina.
4. Cuando el pack tenga una actualización, ResinCraft te avisa; también puedes elegir otra versión del pack en la página de la instancia.

## 5. Cambiar la versión de Minecraft o el cargador de mods

*Desde la versión 1.2.7.*

1. En la página de la instancia, haz clic en la etiqueta de versión de arriba (por ejemplo **1.21.4 · Fabric 0.16.10**). Se abre **Ajustes → Versión**.
2. Elige la versión de **Minecraft**, el **Cargador de mods** y la versión del cargador.
3. Pulsa **Cambiar versión** y confirma. Al iniciar la próxima vez se descarga lo necesario.
4. Puede que los mods hechos para la versión anterior no funcionen: ve a **Contenido** y pulsa **Buscar actualizaciones**.

## 6. Cuando Minecraft se cierra inesperadamente

*Desde la versión 1.2.7.*

Si el juego se cierra de golpe, se abre la ventana **Minecraft se cerró inesperadamente**. ResinCraft lee el informe de error y los registros y te dice qué ha fallado. Por ahora la app muestra estos avisos en inglés, por ejemplo:

- *Sodium Extra needs Sodium, which isn't installed* (Sodium Extra necesita Sodium, que no está instalado) → **Arreglar** instala Sodium.
- *citresewn crashed the game* (citresewn hizo que el juego se cerrara) → **Arreglar** desactiva ese mod (vuelve a activarlo más tarde en **Contenido**).
- *This needs Java 21 or newer* (hace falta Java 21 o más reciente) → **Arreglar** deja que ResinCraft elija el Java adecuado.
- *Minecraft ran out of memory* (Minecraft se quedó sin memoria) → **Arreglar** le da más memoria.

Pulsa **Arreglar** en cada uno, o **Arreglar todo y jugar**. No cambia nada hasta que pulses un botón. Si ResinCraft no sabe la causa, **Abrir registros** te muestra lo último que dijo el juego.

## 7. Copiar cosas de otra instancia

*Desde la versión 1.2.5.*

1. Abre la instancia *a la que* quieres copiar → pulsa el botón **Copiar de otra instancia** de arriba.
2. Elige la instancia *de la que* copiar.
3. Marca lo que quieras:
   - **Mundos**;
   - **Mods** y **Ajustes de mods** (la carpeta config);
   - **Shaders** y **Paquetes de recursos**;
   - **Ajustes del juego** (vídeo, controles, teclas).
4. Pulsa **Copiar**. Los mundos cuyo nombre ya existe reciben un nombre nuevo. Los mods y packs que ya tienes se quedan como están.

## 8. Alojar un servidor

*Desde la versión 0.1.0.*

1. Abre **Servers** → **Create server** (crear servidor).
2. Elige cómo:
   - **Server software** (software de servidor): Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack:** el mismo pack al que juegan tus amigos.
   - **From an instance** (desde una instancia): los mods que ya usas.
   - **From a file** (desde un archivo).
3. Pulsa **Create** (crear). La ventana se cierra y la descarga sigue abajo a la derecha.
4. Acepta el EULA de Minecraft cuando te lo pida y pulsa **Start** (iniciar).

En la página de un servidor:

- **Console** (consola): lo que dice el servidor; escribe comandos abajo.
- **Players** (jugadores): quién está conectado ahora mismo, además de la whitelist, los operadores y los baneos.
- **Mods** o **Plugins:** añadir, actualizar o desactivar. **Actualizar todo** descarga hasta cuatro a la vez; si el servidor está en marcha, las actualizaciones se aplican en el siguiente reinicio.
- **Properties** (propiedades): los ajustes del servidor (dificultad, whitelist activada o no, etc.).
- **Settings** (ajustes):
  - memoria, Java y **Java flags** (los de ResinCraft o los del propio modpack);
  - cómo se conectan tus amigos;
  - el icono del servidor.
- **Reiniciar** y el menú **⋯** (copiar dirección, abrir carpeta, copia de seguridad ahora, eliminar).

- **Settings → Anti-xray** (ajustes → anti-xray; Paper, Purpur, Folia, Leaf): oculta los minerales a los mods y paquetes de texturas de rayos X. **Hide ores** (ocultar minerales) apenas carga el servidor; **Fake ores** (minerales falsos) es más fuerte. Se aplica en el siguiente arranque.
- **Settings → Anti-cheat** (ajustes → anticheat; servidores de plugins): un solo interruptor instala Grim.

Los mods que solo funcionan en el cliente del juego (shaders, minimapas…) se quedan en el servidor mientras arranque bien. Si alguno lo tumba, ResinCraft desactiva ese mod y vuelve a arrancar el servidor; desde la pestaña **Mods** puedes volver a activarlo.

Cuando un modpack no tiene server pack oficial, ResinCraft monta el servidor a partir de la lista de mods del propio pack: la ventana que sale tras **Create** (crear) te lo dice, y el servidor muestra la insignia **Built by ResinCraft** (creado por ResinCraft). Algunos archivos del pack solo se pueden descargar desde la web de su autor. Aparecen arriba en la pestaña **Console**, con **Get file** (conseguir archivo). Descárgalos con tu navegador y ResinCraft los pondrá en su sitio antes del siguiente arranque, o pulsa **Look again** (buscar otra vez).

## 9. Deja que tus amigos entren a tu servidor

*Desde la versión 0.2.0.*

En la pestaña **Settings** del servidor → **How friends connect** (cómo se conectan tus amigos):

- **Automatic** (automático, recomendado y la opción predeterminada para servidores nuevos): primero tu router, luego **ResinCraft Relay** y luego playit.gg.
- **ResinCraft Relay:** tus amigos entran con `yourname.rscraft.resincore.tech`. Necesita una cuenta de Microsoft con la sesión iniciada.
- **playit.gg:** una dirección gratuita que no cambia. La primera vez te pide vincular una cuenta de playit.gg (sirve una cuenta de invitado).
- **Puerto redirigido por mí:** si configuraste el router tú mismo.

La pestaña **Console** muestra la dirección que tienes que dar a tus amigos. Los amigos de Bedrock también pueden entrar a servidores Java: activa **Bedrock players can join** (los jugadores de Bedrock pueden entrar).

**Tu propio dominio:** con ResinCraft Relay, activa **Conectar tu propio dominio a ResinCraft Relay** y escribe el dominio. Añade los registros DNS que muestra, incluido el registro TXT que demuestra que el dominio es tuyo, y pulsa **Verificar**. La insignia pasa de **Verificación pendiente** a **Verificado**. Con el DNS automático de Cloudflare activado, ResinCraft añade los registros por ti.

## 10. Solicitudes de whitelist (página web y bot de Discord)

*Desde la versión 1.2.7.*

Deja que los jugadores pidan entrar, y acéptalos o recházalos tú.

1. En **Properties**, pon **white-list** en **true** (si no, cualquiera puede entrar igualmente).
2. Abre la pestaña **Players** del servidor y activa **Whitelist requests** (solicitudes de whitelist).
3. Elige cómo piden entrar los jugadores:
   - **Web page** (página web):
     - Funciona cuando el servidor se comparte con ResinCraft Relay.
     - Pulsa **Copy link** (copiar enlace) y manda el enlace a los jugadores. Es la propia dirección de tu servidor, o tu dominio verificado.
     - La página funciona mientras el servidor está en línea.
   - **Your Discord bot** (tu bot de Discord):
     1. Abre el [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token**, y copia el token.
     2. Pégalo en ResinCraft y pulsa **Save** (guardar). Se guarda protegido en tu PC y no se vuelve a mostrar.
     3. Cuando ponga **Online as …**, pulsa **Add the bot to your Discord** (añadir el bot a tu Discord) y elige tu servidor de Discord.
     4. Los jugadores escriben `/whitelist TheirName` en tu Discord (con su propio nombre de Minecraft).
4. Las solicitudes aparecen en la pestaña **Players**:
   - **Accept** (aceptar): añade al jugador a la whitelist (el bot se lo dice por mensaje directo).
   - **Reject** (rechazar): rechaza la solicitud.
   - **Reject and block** (rechazar y bloquear): la rechaza y evita que esa persona vuelva a pedirlo. Los jugadores bloqueados aparecen en la lista **Blocked** (bloqueados), con **Unblock** (desbloquear) para cada uno.
   - **Desde la lista de servidores:** los jugadores con ResinCraft pueden pedirlo desde la página de tu servidor en Descubrir → Servidores (sección 17).

Las solicitudes de jugadores baneados se rechazan solas. El bot de Discord funciona mientras ResinCraft está abierto; activa la bandeja del sistema (sección 13) para que siga funcionando con la ventana cerrada.

## 11. Bedrock Edition

*Desde la versión 0.3.1.*

En Windows, **Bedrock**, a la izquierda, instala, abre y desinstala Minecraft para Windows y Minecraft Preview. Antes de desinstalar, hace una copia de seguridad de tus mundos.

## 12. Venir desde otro launcher

*Desde la versión 1.0.5.*

**Biblioteca** → **Traer desde otro launcher** importa instancias de:

- Prism Launcher y MultiMC
- la Modrinth App y la app de CurseForge
- ATLauncher y GDLauncher
- el Minecraft Launcher oficial

Sus mods, mundos y ajustes vienen con ellas. Los originales se quedan donde están.

## 13. Actualizaciones y la bandeja del sistema

*Desde la versión 0.3.0 (la bandeja del sistema desde la 1.2.7).*

- **Actualizaciones:**
  - Cuando hay una versión nueva de ResinCraft lista, aparece un botón **Actualizar** en la barra de título.
  - Si hay un juego o un servidor en marcha, eliges **Cerrarlos y actualizar** (los servidores guardan antes sus mundos) o **Actualizar cuando se detengan**.
- **Actualizar en silencio** (Ajustes → Actualizaciones): instala las versiones nuevas en segundo plano, sin preguntar, cuando no hay nada en marcha.
- **Seguir en la bandeja del sistema** (Ajustes → Apariencia):
  - Al cerrar la ventana, ResinCraft sigue funcionando: los servidores, el bot de Discord y las descargas continúan.
  - Haz clic en el icono de la bandeja para volver a abrirlo; clic derecho → **Quit ResinCraft** (salir de ResinCraft) para cerrarlo del todo.

## 14. Temas y CSS propio

*Desde la versión 1.3.0.*

1. Abre **Ajustes → Apariencia → Tema** y elige una tarjeta: **Sistema**, Oscuro, Claro, Obsidiana, Pizarra profunda, Cerezo, Nether o El End.
2. **CSS propio:** pulsa **Insertar colores actuales**, cambia el color que quieras y luego pulsa **Aplicar**. Funciona sobre cualquier tema; **Borrar** lo quita.
3. Abre **Cómo personalizar** debajo del cuadro: muestra los colores y formas que puedes cambiar, las partes de la app a las que puedes dar estilo y ejemplos (una imagen de fondo, esquinas más redondas, texto más grande…). Haz clic en uno para añadirlo a tu CSS.

Psst: prueba el código Konami (↑ ↑ ↓ ↓ ← → ← → B A), o haz clic siete veces en el logo de ResinCraft.

## 15. Control remoto (consola web)

*Desde la versión 1.3.0.*

Maneja tus servidores desde cualquier navegador, o desde el móvil.

1. Inicia sesión con una cuenta de Microsoft en **Cuentas**.
2. Activa **Ajustes → Control remoto → Permitir control remoto**. Cuando ponga **Conectado como …**, ya está listo.
3. Activa **Seguir en la bandeja del sistema** (sección 13), para que siga funcionando después de cerrar la ventana.
4. Abre `https://console.rscraft.resincore.tech` (o pulsa **Abrir la consola web**) e inicia sesión con **la misma cuenta de Minecraft**.

Desde la consola puedes hacer todo lo que hace el gestor de servidores: iniciar, detener y reiniciar servidores, ver la consola y escribir comandos, gestionar jugadores y solicitudes de whitelist, añadir y actualizar plugins y mods, cambiar propiedades y ajustes, cambiar la versión del modpack, reinstalar, hacer copias de seguridad y crear servidores nuevos. Cada cambio aparece como un aviso en ResinCraft.

El gestor de archivos, los ajustes de Java y los plugins desde cualquier enlace de descarga pueden ejecutar programas en tu PC, así que necesitan un segundo interruptor: **Permitir control total**. Actívalo solo si tu cuenta de Minecraft (Microsoft) tiene la verificación en dos pasos. Sin él, la consola nunca puede llegar a nada más de tu PC.

## 16. Reinstalar y reparar

*Desde la versión 1.3.0.*

- **Una instancia no arranca porque faltan archivos o están dañados:**
  - Haz clic derecho en ella en la **Biblioteca** → **Reinstalar** → **Archivos del juego**, o abre su **Ajustes → Reparar**.
  - Minecraft, el cargador de mods, las bibliotecas y los recursos se vuelven a descargar. Los mundos, mods y ajustes se quedan.
- **Los archivos de un modpack se cambiaron o se borraron:** **Reinstalar** → **Archivos del modpack** vuelve a poner los mods, configuraciones y scripts del pack. Antes hace copia de los archivos que cambiaste; los mundos y tus propios mods se quedan.
- **Un servidor:** su menú **⋯** → **Reinstalar** → **Software del servidor** (o **Archivos del modpack** en un servidor de modpack). El mundo, los ajustes y las listas de jugadores se quedan.
- **El propio ResinCraft:** **Ajustes → Actualizaciones → Reinstalar ResinCraft** vuelve a instalar la app sobre sí misma.

## 17. Encontrar servidores y entrar

*Desde la versión 1.4.0.*

1. Abre **Descubrir** → **Servidores**. Muestra los servidores alojados con ResinCraft y los que han añadido los jugadores.
2. Busca, o filtra por categoría (SMP, PvP, Minijuegos, Skyblock…), región, idioma, modpack, y si pueden entrar jugadores de Bedrock o perfiles sin conexión.
3. Haz clic en el icono o el nombre de un servidor para abrir su página: la descripción completa, todos los detalles, su pack de cliente y enlaces.
4. Pulsa **Entrar**. ResinCraft elige la instancia que encaja con el servidor, o te pregunta cuál usar. Un servidor con sus propios mods tiene **Instalar y entrar**, que crea una instancia con exactamente esos mods y entra.
5. ¿Servidor con whitelist? Pulsa **Pedir entrar**. El botón muestra **Waiting for the host** (esperando al anfitrión), luego **On the whitelist** (en la whitelist; pulsa **Entrar**) o **Ask again** (volver a pedir). ResinCraft te avisa, y hace parpadear la barra de tareas, cuando el anfitrión responde.

Dales me gusta a los servidores que disfrutes, y denuncia los que se salten las normas.

## 18. Pon tu servidor en la lista y comparte sus mods

*Desde la versión 1.4.0.*

1. En la pestaña **Settings** (ajustes) de un servidor alojado, activa **Lista de servidores** (viene desactivada).
2. Añade una descripción, etiquetas, región e idioma. La compatibilidad con Bedrock y con perfiles sin conexión y el modpack se rellenan solos.
3. **Pack de cliente:** publica lo que necesitan los jugadores, para que puedan usar **Instalar y entrar**:
   - el modpack del servidor, instalado desde su propio sitio;
   - cada mod adicional, desde Modrinth o CurseForge;
   - tus propios archivos, enviados directamente desde tu PC mientras ResinCraft está en marcha;
   - los paquetes de recursos y shaders que decidas enviar también.
   Los servidores de plugins y vanilla también pueden tener uno: elige una instancia con la versión de Minecraft del servidor.

¿Un servidor que tienes en otro sitio? Añádelo con **Enviar un servidor** en Descubrir → Servidores.

## 19. Tus otros PC y los amigos que ayudan

*Desde la versión 1.4.1.*

**Tus otros PC:** activa el control remoto en cada PC (sección 15) e inicia sesión con la misma cuenta de Minecraft. Luego, en **Servers**, abre el menú de PC de arriba y elige un PC. Desde aquí puedes iniciar, detener y gestionar sus servidores, y también usar allí **Create server** (crear servidor) o **Importar un servidor**. Para importar desde una carpeta de ese PC, escribe la carpeta; para eso hace falta **Permitir control total** en ese PC.

**Amigos que te ayudan a llevar tus servidores:**

1. Abre **Ajustes → Control remoto → People who can use your servers** (personas que pueden usar tus servidores).
2. Escribe su nombre de Minecraft y pulsa **Add** (añadir).
3. Elige qué pueden hacer:
   - **Viewer** (observador): ver la consola, los jugadores y los ajustes.
   - **Operator** (operador): además, iniciar, detener y reiniciar, enviar comandos y gestionar jugadores y solicitudes de whitelist.
   - **Manager** (gestor): además, cambiar ajustes, plugins y mods, y el modpack.
4. Deja **All servers** (todos los servidores), o elige los servidores en **Which servers?** (¿qué servidores?).

Abren tus servidores desde su ResinCraft (el menú de PC en **Servers**) o desde la consola web, mientras el control remoto está activado y ResinCraft está en marcha en tu PC. Los archivos, los ajustes de Java y crear o borrar servidores siguen siendo cosa tuya.

## 20. Copias de seguridad de los mundos

*Desde la versión 1.4.0.*

- **Automáticas:** mientras un juego o un servidor está en marcha, se hace una copia de seguridad de sus mundos cada 30 minutos y se guardan las 3 más recientes de cada mundo. Cámbialo o desactívalo en **Ajustes → Copia de seguridad automática**.
- **Restaurar:** en la pestaña **Mundos** de una instancia, o en **Settings → Backups** (ajustes → copias de seguridad) en un servidor alojado, pulsa **Restaurar** en una copia, tuya o automática. El mundo que se reemplaza va a la Papelera de reciclaje, así que una restauración se puede deshacer. Un mundo de un jugador también puede volver como copia.

## 21. Almacenamiento y pasar a otro PC

*Desde la versión 1.1.0 (Pasar a otro PC desde la 1.1.1).*

- **Otra unidad:** **Ajustes → Almacenamiento → Cambiar…**, elige una unidad o carpeta y luego:
  - **Mover instancias:** mueve allí tus instancias, servidores y los archivos compartidos de Minecraft.
  - **Usar sin mover:** lo nuevo va allí; tus instancias actuales se quedan y vuelven cuando vuelves a cambiar.
- **Un PC nuevo:** **Ajustes → Pasar a otro PC** guarda todo en un solo archivo `.resincraft`. Ábrelo con ResinCraft en el PC nuevo.

## 22. Solución de problemas

- **El juego no arranca:** mira la ventana **Minecraft se cerró inesperadamente** (sección 6) o la pestaña **Registros** de la instancia. ¿Faltan archivos o están dañados? Reinstálalos (sección 16).
- **Un mod no aparece en Descubrir:** puede que no exista para tu versión de Minecraft o tu cargador. Prueba otra versión, o cambia la versión de la instancia (sección 5).
- **Tus amigos no pueden entrar:** comprueba la dirección en la pestaña **Console** del servidor y que el servidor diga **Running** (en marcha). Con ResinCraft Relay, asegúrate de haber iniciado sesión con una cuenta de Microsoft.
- **El servidor falla al arrancar:** ResinCraft desactiva solo los mods de cliente y lo vuelve a arrancar. Busca en la **Console** la línea que dice qué mod ha desactivado. Si dice que faltan archivos, consíguelos desde la lista de arriba en la pestaña **Console**.
- **¿Sigues atascado?** Informa del problema en [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) con el registro de la pestaña **Registros**.
