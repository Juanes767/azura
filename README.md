<h1><img src="assets/icon.png" width="64" alt="Logo de Azura"> Azura</h1>

**Gestor de fondos de pantalla animados para Linux (X11).** Vídeos e imágenes
como fondo de escritorio, con biblioteca propia, miniaturas, perfiles y
bandeja del sistema. Cada fondo se reproduce con `mpv` incrustado en una
ventana anclada al nivel del escritorio, por debajo de las ventanas normales.

![Plataforma](https://img.shields.io/badge/Plataforma-Linux%20%2F%20X11-blue?logo=linux&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust&logoColor=white)
![GTK 4](https://img.shields.io/badge/GTK-4-4a86cf?logo=gtk&logoColor=white)
![mpv](https://img.shields.io/badge/mpv-reproducci%C3%B3n-691f74?logo=mpv&logoColor=white)
![Licencia](https://img.shields.io/badge/Licencia-MIT-green)

<p align="center">
  <img src="assets/screenshot.png" alt="Captura de Azura con su biblioteca, vista previa y perfiles" width="860">
</p>

## ✨ Características

- 🎞️ **Vídeos e imágenes**: mp4, webm, mkv, mov, gif, png, jpg y jpeg.
- 📚 **Biblioteca**: importar (copiando o con enlace simbólico), buscar, filtrar
  por tipo, favoritos, etiquetas, detección de duplicados y ordenación
  (nombre, fecha, tamaño o manual arrastrando las tarjetas).
- 🖼️ **Miniaturas** generadas con `ffmpeg` y cacheadas por contenido.
- 🔍 **Metadatos**: resolución, duración, fps y códec de cada fondo (ffprobe,
  cacheados).
- ▶️ **Vista previa** integrada antes de aplicar un fondo, con controles de
  pausa, etiquetas, cerrar y aplicar.
- 🔁 **Cambio automático** de fondo con intervalo configurable y orden aleatorio.
- 🎨 **Perfiles**: guarda tu combinación de temas del sistema (GTK, ventanas y
  Cinnamon), la configuración de `fastfetch` y la de `kitty` en un perfil con
  nombre, y aplícala cuando quieras desde su propia página.
- 😴 **Pausa inteligente**: al entrar en pantalla completa, con batería, al pasar
  una app a primer plano, al cambiar de espacio de trabajo, con la pantalla
  bloqueada y por inactividad (sin teclado ni ratón).
- 🎛️ **Límite de FPS** para reducir consumo de GPU.
- 🖥️ **Multi-monitor**: un fondo global o uno distinto por pantalla.
- ⌨️ **Atajos globales** configurables: abrir el menú y cambiar de fondo, con
  alternativas si el atajo elegido está ocupado.
- 🔔 **Notificaciones** al cambiar de fondo (cambio automático, atajo o bandeja).
- 💻 **CLI**: controla el servidor y aplica fondos desde la terminal.
- 📦 **Bandeja del sistema**: menú, aleatorio, siguiente, pausar y salir.
- 🔌 **IPC** por socket JSON para controlar el servidor desde scripts.
- 🔒 **Instancia única** con bloqueo de archivo.
- 🩺 **Vigilancia de mpv**: si el reproductor muere, se relanza solo.

## 📋 Requisitos

- Linux con **X11** (Wayland no está soportado).
- `mpv` y `ffmpeg` en el `PATH`.
- Para compilar: Rust estable y las dependencias de desarrollo de GTK4.

```bash
# Debian/Ubuntu
sudo apt install mpv ffmpeg libgtk-4-dev
# Arch
sudo pacman -S mpv ffmpeg gtk4
```

Probado en Linux Mint 22.3 (Cinnamon, X11).

## 🔨 Compilación

```bash
git clone https://github.com/Juanes767/azura.git
cd azura
cargo build --release
```

Binarios resultantes:

- `target/release/wallpaper-daemon`: servidor que reproduce los fondos.
- `target/release/wallpaper-menu`: interfaz gráfica (GTK4).
- `target/release/wallpaper-animado`: línea de comandos de control.

## 🚀 Uso

```bash
./run.sh          # compila si hace falta e inicia el daemon en primer plano
```

Abre el menú con el binario `wallpaper-menu`, con el lanzador del escritorio o
con el atajo global (`Ctrl+Shift+W` por defecto). Si el servidor no está en
ejecución, el menú muestra el botón **Iniciar servidor**.

### 💻 Línea de comandos

```bash
wallpaper-animado status          # estado del servidor
wallpaper-animado next            # siguiente fondo
wallpaper-animado random          # fondo aleatorio
wallpaper-animado set ~/fondo.mp4 # aplicar un fondo concreto
wallpaper-animado pause           # pausar / resume para reanudar
wallpaper-animado open            # abrir el menú
wallpaper-animado quit            # detener el servidor
wallpaper-animado help            # ayuda completa
```

Códigos de salida: `0` correcto, `1` error, `2` servidor apagado.

### ⚙️ Servicio de usuario (systemd)

```bash
./install-service.sh
systemctl --user status wallpaper-animado   # estado
journalctl --user -u wallpaper-animado -f   # registros
```

El instalador además crea el lanzador «Azura» en el menú de aplicaciones,
instala el icono y enlaza el comando `wallpaper-animado` en `~/.local/bin`.

También se puede activar **Iniciar con la sesión** desde Ajustes, que crea el
`.desktop` de autostart apuntando al daemon.

## ⌨️ Atajos

Presets disponibles (Ajustes → Atajos) para **abrir el menú** y para
**siguiente fondo**:

| Atajo         |
| ------------- |
| Ctrl+Shift+W  |
| Ctrl+Alt+W    |
| Super+W       |
| Ctrl+Alt+F12  |
| Ninguno       |

El daemon intenta registrar el atajo elegido; si ya está en uso, prueba los
demás y reporta el que quedó activo en la interfaz.

## 🖥️ Multi-monitor

En Ajustes se puede fijar un fondo distinto por monitor. El selector
**Pantalla** del destacado indica el destino al aplicar:

- **Todos**: fondo global (limpia los fondos por monitor).
- **Nombre del monitor**: solo esa pantalla.

El cambio automático respeta los fondos ya fijados por monitor.

## 🎨 Perfiles

La página **Perfiles** de la barra lateral guarda la personalización actual y
la restaura cuando quieras:

- **Temas del sistema**: tema GTK, tema de ventanas y tema del escritorio
  (Cinnamon), vía `gsettings`.
- **fastfetch**: `~/.config/fastfetch/config.jsonc` (o `config.json`).
- **kitty**: `~/.config/kitty/kitty.conf`; al aplicar el perfil, kitty recarga
  su configuración al instante (SIGUSR1).

Guardar con un nombre que ya existe pide confirmación para reemplazarlo, y
eliminar un perfil también pide confirmación. Los perfiles se guardan en
`~/.config/wallpaper-animado/perfiles/<nombre>/` con una copia de los archivos
incluidos, así que son portables entre equipos.

## 📁 Rutas

| Qué | Dónde |
| --- | ----- |
| Configuración | `~/.config/wallpaper-animado/config.json` |
| Perfiles | `~/.config/wallpaper-animado/perfiles` |
| Biblioteca | `~/wallpaper-animado/assets/wallpapers` (o `$WALLPAPERS_DIR`) |
| Miniaturas | `~/.cache/wallpaper-animado/thumbs` |
| Socket IPC | `/tmp/wallpaper-animado.sock` |

## 🔌 IPC

El daemon escucha en `/tmp/wallpaper-animado.sock` líneas JSON. Siempre
responde una línea JSON. Ejemplos:

```bash
printf '{"cmd":"status"}\n' | socat - UNIX-CONNECT:/tmp/wallpaper-animado.sock
printf '{"cmd":"set_wallpaper","path":"/ruta/fondo.mp4","monitor":"DP-1"}\n' \
  | socat - UNIX-CONNECT:/tmp/wallpaper-animado.sock
```

Comandos: `ping`, `status`, `set_wallpaper`, `clear`, `pause`, `resume`,
`toggle_pause`, `siguiente`, `aleatorio`, `show_menu`, `save_config` y `quit`.
El comando `wallpaper-animado` de la CLI envuelve estos mensajes; `status`
incluye además el estado de pantalla bloqueada y el atajo de siguiente.

## 🗂️ Estructura

```
src/
  core.rs    # biblioteca compartida: config, IPC, utilidades, tests
  daemon.rs  # servidor: mpv, multi-monitor, atajos, bandeja, IPC
  menu.rs    # interfaz GTK4
  cli.rs     # línea de comandos
assets/
  icon.png        # logo (512x512)
  icons/          # icono de la aplicación por tamaños (tema hicolor)
  screenshot.png  # captura de la aplicación
  wallpapers/     # medios de ejemplo (no versionados)
```

## 🛠️ Desarrollo

```bash
cargo fmt      # formato
cargo clippy   # linter
cargo test     # pruebas de la biblioteca compartida
```

## ⚠️ Limitaciones

- Solo X11: la incrustación de mpv usa el XID de la ventana. En Wayland el
  fondo no se puede anclar al escritorio; usa un compositor con soporte de
  protocolos de fondo (p. ej. `swww`) o XWayland como alternativa parcial.
- Cambiar el fondo por monitor requiere RandR (X11 1.5+).
- Los perfiles solo capturan los temas de Cinnamon, `fastfetch` y `kitty`; si
  alguno de esos archivos no existe al guardar, el perfil no incluye esa parte
  y al aplicarlo no se toca.

## 📄 Licencia

Distribuido bajo la licencia [MIT](LICENSE).
