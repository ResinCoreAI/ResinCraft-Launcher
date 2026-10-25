# ResinCraft Launcher: guía de uso

Cómo hacer lo más habitual en ResinCraft. Los nombres en **negrita** son lo que verás en pantalla. Algunas partes de la página de servidores todavía no están traducidas y la app las muestra en inglés; por eso aquí también aparecen en inglés (con la traducción entre paréntesis).

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
17. [Solución de problemas](#17-solución-de-problemas)

## 1. Instalar e iniciar sesión

1. Descarga ResinCraft para tu sistema desde la [página de versiones](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** [`ResinCraft-Setup.exe`](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/download/setup/ResinCraft-Setup.exe) (siempre instala la versión más reciente), o `…_amd64_windows.exe` / `…_amd64_windows.msi`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` o `.tar.gz`
2. Instálalo y ábrelo. Puede que Windows diga que el editor no está verificado; en macOS, la primera vez haz clic derecho en la app y elige **Abrir**.
3. Abre **Cuentas** → **Iniciar sesión con Microsoft** e inicia sesión en la propia página de Microsoft. Tu contraseña nunca pasa por ResinCraft.
   - ¿No tienes cuenta de Microsoft? Añade un **Perfil sin conexión** para jugar solo y en LAN.

## 2. Crea tu primera instancia

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

1. Abre tu instancia → pestaña **Contenido** → **Añadir mods**. O abre **Descubrir** en la barra de la izquierda.
2. Busca, o explora Modrinth, CurseForge, FTB y los demás sitios. Solo se muestran las versiones que encajan con tu instancia.
3. Pulsa **Instalar**. Los mods necesarios (dependencias) se añaden solos.
4. De vuelta en **Contenido**:
   - usa el interruptor junto a un mod para activarlo o desactivarlo;
   - pulsa **Buscar actualizaciones** para actualizar mods (uno o todos).

Algunos autores de CurseForge no permiten que los launchers descarguen sus archivos. En ese caso, ResinCraft los muestra en **Descargar todo**: descárgalos con tu navegador y ResinCraft los encontrará solo en tu carpeta de descargas. ¿Tu navegador guarda en otro sitio? Añade esa carpeta en **Ajustes → Descargas → Carpetas vigiladas**.

## 4. Instalar un modpack

1. Abre **Descubrir** → **Modpacks** (o **Biblioteca** → **Nueva instancia** → **Descargar un modpack**).
2. Abre un pack para ver su descripción, sus versiones y los **Mods** que contiene.
3. Pulsa **Instalar pack**. La instancia nueva aparece enseguida en la **Biblioteca**, con el progreso de la instalación en su tarjeta; **Jugar** funciona cuando termina.
4. Cuando el pack tenga una actualización, ResinCraft te avisa; también puedes elegir otra versión del pack en la página de la instancia.

## 5. Cambiar la versión de Minecraft o el cargador de mods

1. En la página de la instancia, haz clic en la etiqueta de versión de arriba (por ejemplo **1.21.4 · Fabric 0.16.10**). Se abre **Ajustes → Versión**.
2. Elige la versión de **Minecraft**, el **Cargador de mods** y la versión del cargador.
3. Pulsa **Cambiar versión** y confirma. Al iniciar la próxima vez se descarga lo necesario.
4. Puede que los mods hechos para la versión anterior no funcionen: ve a **Contenido** y pulsa **Buscar actualizaciones**.

## 6. Cuando Minecraft se cierra inesperadamente

Si el juego se cierra de golpe, se abre la ventana **Minecraft se cerró inesperadamente**. ResinCraft lee el informe de error y los registros y te dice qué ha fallado. Por ahora la app muestra estos avisos en inglés, por ejemplo:

- *Sodium Extra needs Sodium, which isn't installed* (Sodium Extra necesita Sodium, que no está instalado) → **Arreglar** instala Sodium.
- *citresewn crashed the game* (citresewn hizo que el juego se cerrara) → **Arreglar** desactiva ese mod (vuelve a activarlo más tarde en **Contenido**).
- *This needs Java 21 or newer* (hace falta Java 21 o más reciente) → **Arreglar** deja que ResinCraft elija el Java adecuado.
- *Minecraft ran out of memory* (Minecraft se quedó sin memoria) → **Arreglar** le da más memoria.

Pulsa **Arreglar** en cada uno, o **Arreglar todo y jugar**. No cambia nada hasta que pulses un botón. Si ResinCraft no sabe la causa, **Abrir registros** te muestra lo último que dijo el juego.

## 7. Copiar cosas de otra instancia

1. Abre la instancia *a la que* quieres copiar → pulsa el botón **Copiar de otra instancia** de arriba.
2. Elige la instancia *de la que* copiar.
3. Marca lo que quieras:
   - **Mundos**;
   - **Mods** y **Ajustes de mods** (la carpeta config);
   - **Shaders** y **Paquetes de recursos**;
   - **Ajustes del juego** (vídeo, controles, teclas).
4. Pulsa **Copiar**. Los mundos cuyo nombre ya existe reciben un nombre nuevo. Los mods y packs que ya tienes se quedan como están.

## 8. Alojar un servidor

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
- **Mods** o **Plugins:** añadir, actualizar o desactivar.
- **Properties** (propiedades): los ajustes del servidor (dificultad, whitelist activada o no, etc.).
- **Settings** (ajustes):
  - memoria, Java y **Java flags** (los de ResinCraft o los del propio modpack);
  - cómo se conectan tus amigos;
  - el icono del servidor.
- **Reiniciar** y el menú **⋯** (copiar dirección, abrir carpeta, copia de seguridad ahora, eliminar).

- **Settings → Anti-xray** (ajustes → anti-xray; Paper, Purpur, Folia, Leaf): oculta los minerales a los mods y paquetes de texturas de rayos X. **Hide ores** (ocultar minerales) apenas carga el servidor; **Fake ores** (minerales falsos) es más fuerte. Se aplica en el siguiente arranque.

Los mods que solo funcionan en el cliente del juego (shaders, minimapas…) se quedan en el servidor mientras arranque bien. Si alguno lo tumba, ResinCraft desactiva ese mod y vuelve a arrancar el servidor; desde la pestaña **Mods** puedes volver a activarlo.

Cuando un modpack no tiene server pack oficial, ResinCraft monta el servidor a partir de la lista de mods del propio pack: la ventana que sale tras **Create** (crear) te lo dice, y el servidor muestra la insignia **Built by ResinCraft** (creado por ResinCraft). Algunos archivos del pack solo se pueden descargar desde la web de su autor. Aparecen arriba en la pestaña **Console**, con **Get file** (conseguir archivo). Descárgalos con tu navegador y ResinCraft los pondrá en su sitio antes del siguiente arranque, o pulsa **Look again** (buscar otra vez).

## 9. Deja que tus amigos entren a tu servidor

En la pestaña **Settings** del servidor → **How friends connect** (cómo se conectan tus amigos):

- **Automatic** (automático, recomendado): prueba primero **ResinCraft Relay**, luego playit.gg y luego bore.
- **ResinCraft Relay:** tus amigos entran con `yourname.rscraft.resincore.tech`. Necesita una cuenta de Microsoft con la sesión iniciada.
- **playit.gg:** una dirección gratuita que no cambia. La primera vez te pide vincular una cuenta de playit.gg (sirve una cuenta de invitado).
- **Puerto redirigido por mí:** si configuraste el router tú mismo.

La pestaña **Console** muestra la dirección que tienes que dar a tus amigos. Los amigos de Bedrock también pueden entrar a servidores Java: activa **Bedrock players can join** (los jugadores de Bedrock pueden entrar).

## 10. Solicitudes de whitelist (página web y bot de Discord)

Deja que los jugadores pidan entrar, y acéptalos o recházalos tú.

1. En **Properties**, pon **white-list** en **true** (si no, cualquiera puede entrar igualmente).
2. Abre la pestaña **Players** del servidor y activa **Whitelist requests** (solicitudes de whitelist).
3. Elige cómo piden entrar los jugadores:
   - **Web page** (página web):
     - Funciona cuando el servidor se comparte con ResinCraft Relay.
     - Pulsa **Copy link** (copiar enlace) y manda el enlace a los jugadores: `https://rscraft.resincore.tech/join/<name>`.
     - La página funciona mientras el servidor está en línea.
   - **Your Discord bot** (tu bot de Discord):
     1. Abre el [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token**, y copia el token.
     2. Pégalo en ResinCraft y pulsa **Save** (guardar). Se guarda protegido en tu PC y no se vuelve a mostrar.
     3. Cuando ponga **Online as …**, pulsa **Add the bot to your Discord** (añadir el bot a tu Discord) y elige tu servidor de Discord.
     4. Los jugadores escriben `/whitelist TheirName` en tu Discord (con su propio nombre de Minecraft).
4. Las solicitudes aparecen en la pestaña **Players**:
   - **Accept** (aceptar): añade al jugador a la whitelist (el bot se lo dice por mensaje directo).
   - **Reject** (rechazar): rechaza la solicitud.
   - **Reject and block** (rechazar y bloquear): la rechaza y evita que esa persona vuelva a pedirlo. **Unblock all** (desbloquear a todos) deshace los bloqueos de Discord.

Las solicitudes de jugadores baneados se rechazan solas. El bot de Discord funciona mientras ResinCraft está abierto; activa la bandeja del sistema (sección 13) para que siga funcionando con la ventana cerrada.

## 11. Bedrock Edition

En Windows, **Bedrock**, a la izquierda, instala, abre y desinstala Minecraft para Windows y Minecraft Preview. Antes de desinstalar, hace una copia de seguridad de tus mundos.

## 12. Venir desde otro launcher

**Biblioteca** → **Traer desde otro launcher** importa instancias de:

- Prism Launcher y MultiMC
- la Modrinth App y la app de CurseForge
- ATLauncher y GDLauncher
- el Minecraft Launcher oficial

Sus mods, mundos y ajustes vienen con ellas. Los originales se quedan donde están.

## 13. Actualizaciones y la bandeja del sistema

- **Actualizaciones:**
  - Cuando hay una versión nueva de ResinCraft lista, aparece un botón **Actualizar** en la barra de título.
  - Si hay un juego o un servidor en marcha, eliges **Cerrarlos y actualizar** (los servidores guardan antes sus mundos) o **Actualizar cuando se detengan**.
- **Actualizar en silencio** (Ajustes → Actualizaciones): instala las versiones nuevas en segundo plano, sin preguntar, cuando no hay nada en marcha.
- **Seguir en la bandeja del sistema** (Ajustes → Apariencia):
  - Al cerrar la ventana, ResinCraft sigue funcionando: los servidores, el bot de Discord y las descargas continúan.
  - Haz clic en el icono de la bandeja para volver a abrirlo; clic derecho → **Quit ResinCraft** (salir de ResinCraft) para cerrarlo del todo.

## 14. Temas y CSS propio

1. Abre **Ajustes → Apariencia → Tema** y elige una tarjeta: **Sistema**, Oscuro, Claro, Obsidiana, Pizarra profunda, Cerezo, Nether o El End.
2. **CSS propio:** pulsa **Insertar colores actuales**, cambia el color que quieras y luego pulsa **Aplicar**. Funciona sobre cualquier tema; **Borrar** lo quita.
3. Abre **Cómo personalizar** debajo del cuadro: muestra los colores y formas que puedes cambiar, las partes de la app a las que puedes dar estilo y ejemplos (una imagen de fondo, esquinas más redondas, texto más grande…). Haz clic en uno para añadirlo a tu CSS.

Psst: prueba el código Konami (↑ ↑ ↓ ↓ ← → ← → B A), o haz clic siete veces en el logo de ResinCraft.

## 15. Control remoto (consola web)

Maneja tus servidores desde cualquier navegador, o desde el móvil.

1. Inicia sesión con una cuenta de Microsoft en **Cuentas**.
2. Activa **Ajustes → Control remoto → Permitir control remoto**. Cuando ponga **Conectado como …**, ya está listo.
3. Activa **Seguir en la bandeja del sistema** (sección 13), para que siga funcionando después de cerrar la ventana.
4. Abre `https://console.rscraft.resincore.tech` (o pulsa **Abrir la consola web**) e inicia sesión con **la misma cuenta de Minecraft**.

Desde la consola puedes hacer todo lo que hace el gestor de servidores: iniciar, detener y reiniciar servidores, ver la consola y escribir comandos, gestionar jugadores y solicitudes de whitelist, añadir y actualizar plugins y mods, cambiar propiedades y ajustes, cambiar la versión del modpack, reinstalar, hacer copias de seguridad y crear servidores nuevos. Cada cambio aparece como un aviso en ResinCraft.

El gestor de archivos, los ajustes de Java y los plugins desde cualquier enlace de descarga pueden ejecutar programas en tu PC, así que necesitan un segundo interruptor: **Permitir control total**. Actívalo solo si tu cuenta de Minecraft (Microsoft) tiene la verificación en dos pasos. Sin él, la consola nunca puede llegar a nada más de tu PC.

## 16. Reinstalar y reparar

- **Una instancia no arranca porque faltan archivos o están dañados:**
  - Haz clic derecho en ella en la **Biblioteca** → **Reinstalar** → **Archivos del juego**, o abre su **Ajustes → Reparar**.
  - Minecraft, el cargador de mods, las bibliotecas y los recursos se vuelven a descargar. Los mundos, mods y ajustes se quedan.
- **Los archivos de un modpack se cambiaron o se borraron:** **Reinstalar** → **Archivos del modpack** vuelve a poner los mods, configuraciones y scripts del pack. Antes hace copia de los archivos que cambiaste; los mundos y tus propios mods se quedan.
- **Un servidor:** su menú **⋯** → **Reinstalar** → **Software del servidor** (o **Archivos del modpack** en un servidor de modpack). El mundo, los ajustes y las listas de jugadores se quedan.
- **El propio ResinCraft:** **Ajustes → Actualizaciones → Reinstalar ResinCraft** vuelve a instalar la app sobre sí misma.

## 17. Solución de problemas

- **El juego no arranca:** mira la ventana **Minecraft se cerró inesperadamente** (sección 6) o la pestaña **Registros** de la instancia. ¿Faltan archivos o están dañados? Reinstálalos (sección 16).
- **Un mod no aparece en Descubrir:** puede que no exista para tu versión de Minecraft o tu cargador. Prueba otra versión, o cambia la versión de la instancia (sección 5).
- **Tus amigos no pueden entrar:** comprueba la dirección en la pestaña **Console** del servidor y que el servidor diga **Running** (en marcha). Con ResinCraft Relay, asegúrate de haber iniciado sesión con una cuenta de Microsoft.
- **El servidor falla al arrancar:** ResinCraft desactiva solo los mods de cliente y lo vuelve a arrancar. Busca en la **Console** la línea que dice qué mod ha desactivado. Si dice que faltan archivos, consíguelos desde la lista de arriba en la pestaña **Console**.
- **¿Sigues atascado?** Informa del problema en [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) con el registro de la pestaña **Registros**.
