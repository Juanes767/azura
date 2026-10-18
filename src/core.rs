// Lógica compartida entre el daemon y el menú
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

/// Directorio privado (0700) donde viven los sockets de la aplicación.
///
/// Prefiere `$XDG_RUNTIME_DIR` (por usuario, ya es 0700); si no existe, cae al
/// directorio de caché y, en último caso, al temporal del sistema. Así otros
/// usuarios locales no pueden precrear ni conectar con nuestros sockets.
pub fn dir_sockets() -> PathBuf {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    let base = dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("wallpaper-animado");
    let _ = std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&base);
    // Si el directorio ya existía, asegura igualmente los permisos privados.
    let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
    base
}

/// Ruta del socket IPC del daemon.
pub fn socket_ipc() -> PathBuf {
    dir_sockets().join("daemon.sock")
}

/// Elimina `ruta` solo si es un socket Unix. Nunca borra archivos ni symlinks.
pub fn eliminar_socket(ruta: &Path) {
    use std::os::unix::fs::FileTypeExt;

    if let Ok(meta) = std::fs::symlink_metadata(ruta) {
        if meta.file_type().is_socket() {
            let _ = std::fs::remove_file(ruta);
        }
    }
}

/// Presets de atajo global: (id, descripción legible).
pub const HOTKEYS: [(&str, &str); 5] = [
    ("ctrl-shift-w", "Ctrl+Shift+W"),
    ("ctrl-alt-w", "Ctrl+Alt+W"),
    ("super-w", "Super+W"),
    ("ctrl-alt-f12", "Ctrl+Alt+F12"),
    ("ninguno", "Ninguno"),
];

/// Descripción legible de un preset de atajo.
pub fn hotkey_descripcion(id: &str) -> String {
    HOTKEYS
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| (*v).to_string())
        .unwrap_or_else(|| HOTKEYS[0].1.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Vídeo activo (ruta absoluta)
    pub current_wallpaper: Option<String>,
    /// Ejecutar el daemon al iniciar la sesión
    pub autostart: bool,
    /// Pausar el fondo cuando una app normal está en fullscreen con foco
    pub pausa_fullscreen: bool,
    /// Pausar el fondo cuando el equipo va con batería
    pub pausa_bateria: bool,
    /// Pausar el fondo mientras una aplicación tiene el foco (primer plano)
    pub pausa_app: bool,
    /// Pausar el fondo al cambiar de espacio de trabajo
    pub pausa_workspace: bool,
    /// Límite de FPS de mpv (0 = sin límite)
    pub fps_limit: u32,
    /// Cambiar de fondo automáticamente
    pub slideshow: bool,
    /// Intervalo del slideshow en minutos
    pub slideshow_minutos: u32,
    /// Elegir el siguiente fondo al azar en vez de en orden alfabético
    pub slideshow_aleatorio: bool,
    /// Atajo global para abrir el menú (id de HOTKEYS)
    pub hotkey: String,
    /// Atajo global para pasar al siguiente fondo (id de HOTKEYS; "ninguno" = sin atajo)
    pub hotkey_siguiente: String,
    /// Pausar el fondo mientras la pantalla está bloqueada
    pub pausa_bloqueo: bool,
    /// Pausar el fondo tras un tiempo sin actividad
    pub pausa_inactividad: bool,
    /// Minutos sin actividad antes de pausar
    pub pausa_inactividad_minutos: u32,
    /// Rutas de los fondos marcados como favoritos
    pub favoritos: Vec<String>,
    /// Al añadir fondos, enlazarlos en vez de copiarlos
    pub enlazar_al_importar: bool,
    /// Orden de la biblioteca: nombre-az | nombre-za | fecha | tamano | manual
    pub orden_biblioteca: String,
    /// Orden personalizado de la biblioteca (rutas)
    pub orden_manual: Vec<String>,
    /// Etiquetas por fondo (ruta → etiquetas)
    pub etiquetas: HashMap<String, Vec<String>>,
    /// Fondo fijado por monitor (nombre del monitor → ruta)
    pub fondos_por_monitor: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            current_wallpaper: None,
            autostart: false,
            pausa_fullscreen: true,
            pausa_bateria: true,
            pausa_app: true,
            pausa_workspace: true,
            fps_limit: 0,
            slideshow: false,
            slideshow_minutos: 30,
            slideshow_aleatorio: false,
            hotkey: HOTKEYS[0].0.to_string(),
            hotkey_siguiente: "ninguno".to_string(),
            pausa_bloqueo: true,
            pausa_inactividad: false,
            pausa_inactividad_minutos: 15,
            favoritos: Vec::new(),
            enlazar_al_importar: false,
            orden_biblioteca: "nombre-az".to_string(),
            orden_manual: Vec::new(),
            etiquetas: HashMap::new(),
            fondos_por_monitor: HashMap::new(),
        }
    }
}

impl Config {
    pub fn ruta() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_default()
            .join("wallpaper-animado/config.json")
    }

    pub fn cargar() -> Self {
        Self::cargar_de(&Self::ruta())
    }

    pub fn cargar_de(ruta: &Path) -> Self {
        std::fs::read_to_string(ruta)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn guardar(&self) -> std::io::Result<()> {
        self.guardar_en(&Self::ruta())
    }

    pub fn guardar_en(&self, ruta: &Path) -> std::io::Result<()> {
        if let Some(dir) = ruta.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        escribir_atomico(ruta, &json)
    }
}

/// Escribe `contenido` en `ruta` de forma atómica (temporal + rename), para
/// no dejar el archivo a medias si el proceso se interrumpe a mitad de escritura.
fn escribir_atomico(ruta: &Path, contenido: &str) -> std::io::Result<()> {
    let mut nombre = ruta.as_os_str().to_owned();
    nombre.push(format!(".tmp-{}", std::process::id()));
    let temporal = PathBuf::from(nombre);

    std::fs::write(&temporal, contenido)?;
    if let Err(e) = std::fs::rename(&temporal, ruta) {
        let _ = std::fs::remove_file(&temporal);
        return Err(e);
    }
    Ok(())
}

/// Estado que el daemon reporta al menú a través del comando IPC `status`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EstadoDaemon {
    pub version: String,
    pub wallpaper: Option<String>,
    pub pausa_manual: bool,
    pub hotkey: String,
    pub hotkey_siguiente: String,
    pub pantalla_bloqueada: bool,
    pub mpv_disponible: bool,
    pub ffmpeg_disponible: bool,
    pub monitores: Vec<String>,
    pub fondos_por_monitor: HashMap<String, String>,
    pub uptime_seg: u64,
}

/// Metadatos de un fondo extraídos con ffprobe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetaFondo {
    pub ancho: u32,
    pub alto: u32,
    pub duracion_seg: f64,
    pub fps: f64,
    pub codec: String,
}

impl MetaFondo {
    /// Resolución abreviada: «1080p» o «1920x1080» si no es un ancho típico.
    pub fn resolucion(&self) -> String {
        let alto_comun = [480, 720, 1080, 1440, 2160];
        if self.ancho > 0 && self.alto > 0 && alto_comun.contains(&self.alto) {
            format!("{}p", self.alto)
        } else {
            format!("{}x{}", self.ancho, self.alto)
        }
    }

    /// Duración en formato «m:ss» (o «h:mm:ss»).
    pub fn duracion(&self) -> String {
        let total = self.duracion_seg.max(0.0).round() as u64;
        let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
        if h > 0 {
            format!("{}:{:02}:{:02}", h, m, s)
        } else {
            format!("{}:{:02}", m, s)
        }
    }

    /// Línea compacta para la vista previa: «1080p · 30 fps · 0:45 · h264».
    pub fn descripcion(&self) -> String {
        let mut partes = vec![self.resolucion()];
        if self.fps > 0.0 {
            partes.push(format!("{:.0} fps", self.fps));
        }
        if self.duracion_seg > 0.0 {
            partes.push(self.duracion());
        }
        if !self.codec.is_empty() {
            partes.push(self.codec.clone());
        }
        partes.join(" · ")
    }
}

/// Interpreta la salida JSON de `ffprobe -show_format -show_streams`.
pub fn parsear_ffprobe(json: &str) -> Option<MetaFondo> {
    let valor: serde_json::Value = serde_json::from_str(json).ok()?;
    let streams = valor.get("streams")?.as_array()?;
    let video = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(|t| t.as_str()) == Some("video"))?;

    let ancho = video.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let alto = video.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let codec = video
        .get("codec_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let fps = video
        .get("avg_frame_rate")
        .and_then(|v| v.as_str())
        .and_then(parsear_fraccion)
        .filter(|f| *f > 0.0)
        .or_else(|| {
            video
                .get("r_frame_rate")
                .and_then(|v| v.as_str())
                .and_then(parsear_fraccion)
        })
        .unwrap_or(0.0);

    let duracion_seg = video
        .get("duration")
        .and_then(|v| v.as_str())
        .and_then(|d| d.parse::<f64>().ok())
        .or_else(|| {
            valor
                .get("format")
                .and_then(|f| f.get("duration"))
                .and_then(|v| v.as_str())
                .and_then(|d| d.parse::<f64>().ok())
        })
        .unwrap_or(0.0);

    Some(MetaFondo {
        ancho,
        alto,
        duracion_seg,
        fps,
        codec,
    })
}

fn parsear_fraccion(texto: &str) -> Option<f64> {
    let (num, den) = texto.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

/// Ejecuta ffprobe y devuelve los metadatos del archivo (None si falla).
pub fn leer_metadatos(path: &Path) -> Option<MetaFondo> {
    let salida = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    parsear_ffprobe(&String::from_utf8_lossy(&salida.stdout))
}

/// Huella rápida de contenido: tamaño + primeros y últimos 256 KB.
/// Sirve para detectar duplicados sin leer archivos enteros.
pub fn hash_rapido(path: &Path) -> Option<u64> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::io::{Read, Seek, SeekFrom};

    const BLOQUE: u64 = 256 * 1024;

    let mut archivo = std::fs::File::open(path).ok()?;
    let tamano = archivo.metadata().ok()?.len();

    let mut h = DefaultHasher::new();
    tamano.hash(&mut h);

    let mut buf = vec![0u8; BLOQUE as usize];
    let n = archivo.read(&mut buf).ok()?;
    buf[..n].hash(&mut h);

    if tamano > BLOQUE {
        archivo
            .seek(SeekFrom::Start(tamano.saturating_sub(BLOQUE)))
            .ok()?;
        let n = archivo.read(&mut buf).ok()?;
        buf[..n].hash(&mut h);
    }

    Some(h.finish())
}

/// Lee una clave de gsettings («schema clave»), sin comillas.
pub fn gsettings_get(clave: &str) -> Option<String> {
    let partes: Vec<&str> = clave.split_whitespace().collect();
    let salida = std::process::Command::new("gsettings")
        .arg("get")
        .args(&partes)
        .output()
        .ok()?;
    salida.status.success().then(|| {
        String::from_utf8_lossy(&salida.stdout)
            .trim()
            .trim_matches('\'')
            .to_string()
    })
}

/// Escribe una clave de gsettings. Devuelve true si se aplicó.
pub fn gsettings_set(clave: &str, valor: &str) -> bool {
    let partes: Vec<&str> = clave.split_whitespace().collect();
    std::process::Command::new("gsettings")
        .arg("set")
        .args(&partes)
        // «--» evita que un valor que empiece por «-» se interprete como opción.
        .arg("--")
        .arg(valor)
        .status()
        .map(|e| e.success())
        .unwrap_or(false)
}

// ── Perfiles (temas del sistema + fastfetch + kitty) ───────────

/// Claves de gsettings que forman el «tema del sistema» del perfil.
pub const GS_TEMA_GTK: &str = "org.cinnamon.desktop.interface gtk-theme";
pub const GS_TEMA_WM: &str = "org.cinnamon.desktop.wm.preferences theme";
pub const GS_TEMA_CINNAMON: &str = "org.cinnamon.theme name";

/// Perfil de personalización guardado por el usuario.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Perfil {
    pub nombre: String,
    #[serde(default)]
    pub gtk: Option<String>,
    #[serde(default)]
    pub wm: Option<String>,
    #[serde(default)]
    pub cinnamon: Option<String>,
    /// Nombre del archivo original de fastfetch («config.jsonc» o «config.json»)
    #[serde(default)]
    pub fastfetch: Option<String>,
    /// «kitty.conf» si el perfil incluye la configuración de kitty
    #[serde(default)]
    pub kitty: Option<String>,
}

/// Nombre de carpeta seguro a partir del nombre del perfil.
pub fn slug(nombre: &str) -> String {
    let mut s = String::new();
    for c in nombre.chars() {
        if c.is_alphanumeric() {
            s.extend(c.to_lowercase());
        } else if !s.is_empty() && !s.ends_with('-') {
            s.push('-');
        }
    }
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() {
        "perfil".to_string()
    } else {
        s
    }
}

pub fn dir_perfiles() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("wallpaper-animado/perfiles")
}

/// Comprueba que `nombre` es un nombre de archivo simple: sin directorios,
/// sin «..» y sin separadores. Evita rutas maliciosas en perfiles manipulados.
fn nombre_archivo_seguro(nombre: &str) -> bool {
    matches!(
        Path::new(nombre)
            .components()
            .collect::<Vec<_>>()
            .as_slice(),
        [std::path::Component::Normal(_)]
    )
}

/// Extensión admitida para la configuración de fastfetch de un perfil.
fn extension_fastfetch(nombre: &str) -> Option<&'static str> {
    match Path::new(nombre).extension().and_then(|e| e.to_str()) {
        Some("json") => Some("json"),
        Some("jsonc") => Some("jsonc"),
        _ => None,
    }
}

impl Perfil {
    pub fn dir(&self) -> PathBuf {
        dir_perfiles().join(slug(&self.nombre))
    }
}

/// Ruta del archivo de configuración de fastfetch del usuario.
fn ruta_fastfetch() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("fastfetch");
    for nombre in ["config.jsonc", "config.json"] {
        let ruta = dir.join(nombre);
        if ruta.is_file() {
            return Some(ruta);
        }
    }
    None
}

/// Ruta del archivo de configuración de kitty del usuario.
fn ruta_kitty() -> Option<PathBuf> {
    let ruta = dirs::config_dir()?.join("kitty/kitty.conf");
    ruta.is_file().then_some(ruta)
}

/// Guarda la configuración actual (temas, fastfetch, kitty) como perfil.
pub fn guardar_perfil(nombre: &str) -> std::io::Result<Perfil> {
    let mut perfil = Perfil {
        nombre: nombre.trim().to_string(),
        ..Perfil::default()
    };
    let dir = perfil.dir();
    std::fs::create_dir_all(&dir)?;

    perfil.gtk = gsettings_get(GS_TEMA_GTK);
    perfil.wm = gsettings_get(GS_TEMA_WM);
    perfil.cinnamon = gsettings_get(GS_TEMA_CINNAMON);

    if let Some(origen) = ruta_fastfetch() {
        let archivo = origen
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "config.jsonc".to_string());
        let ext = origen
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_else(|| "jsonc".to_string());
        std::fs::copy(&origen, dir.join(format!("fastfetch.{ext}")))?;
        perfil.fastfetch = Some(archivo);
    }

    if let Some(origen) = ruta_kitty() {
        std::fs::copy(&origen, dir.join("kitty.conf"))?;
        perfil.kitty = Some("kitty.conf".to_string());
    }

    let texto = serde_json::to_string_pretty(&perfil).map_err(std::io::Error::other)?;
    escribir_atomico(&dir.join("perfil.json"), &texto)?;
    Ok(perfil)
}

/// Perfiles guardados, ordenados por nombre.
pub fn listar_perfiles() -> Vec<Perfil> {
    let mut perfiles = Vec::new();
    let Ok(entradas) = std::fs::read_dir(dir_perfiles()) else {
        return perfiles;
    };
    for entrada in entradas.flatten() {
        let Ok(texto) = std::fs::read_to_string(entrada.path().join("perfil.json")) else {
            continue;
        };
        if let Ok(perfil) = serde_json::from_str::<Perfil>(&texto) {
            perfiles.push(perfil);
        }
    }
    perfiles.sort_by_key(|p| p.nombre.to_lowercase());
    perfiles
}

/// Aplica un perfil: temas del sistema, fastfetch, kitty y recarga de kitty.
pub fn aplicar_perfil(perfil: &Perfil) -> std::io::Result<()> {
    let dir = perfil.dir();

    if let Some(v) = &perfil.gtk {
        gsettings_set(GS_TEMA_GTK, v);
    }
    if let Some(v) = &perfil.wm {
        gsettings_set(GS_TEMA_WM, v);
    }
    if let Some(v) = &perfil.cinnamon {
        gsettings_set(GS_TEMA_CINNAMON, v);
    }

    if let Some(archivo) = &perfil.fastfetch {
        // El perfil es un archivo de usuario: nunca confiar en rutas ni extensiones.
        let Some(ext) = extension_fastfetch(archivo) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("extensión no admitida en el perfil: {}", archivo),
            ));
        };
        if !nombre_archivo_seguro(archivo) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("nombre de archivo no válido en el perfil: {}", archivo),
            ));
        }
        let destino = dirs::config_dir().unwrap_or_default().join("fastfetch");
        std::fs::create_dir_all(&destino)?;
        std::fs::copy(dir.join(format!("fastfetch.{ext}")), destino.join(archivo))?;
    }

    if perfil.kitty.is_some() {
        let destino = dirs::config_dir().unwrap_or_default().join("kitty");
        std::fs::create_dir_all(&destino)?;
        std::fs::copy(dir.join("kitty.conf"), destino.join("kitty.conf"))?;
        recargar_kitty();
    }

    Ok(())
}

/// Pide a kitty recargar su configuración (SIGUSR1). Silencioso si no corre.
fn recargar_kitty() {
    let _ = std::process::Command::new("pkill")
        .args(["-USR1", "-x", "kitty"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Elimina un perfil guardado del disco.
pub fn eliminar_perfil(perfil: &Perfil) -> std::io::Result<()> {
    std::fs::remove_dir_all(perfil.dir())
}

pub fn wallpapers_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("WALLPAPERS_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join("wallpaper-animado/assets/wallpapers")
}

pub fn cache_dir() -> PathBuf {
    let dir = dirs::cache_dir()
        .unwrap_or_default()
        .join("wallpaper-animado/thumbs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn extension_ok(path: &Path) -> bool {
    const EXTS: [&str; 8] = ["mp4", "webm", "mkv", "mov", "gif", "png", "jpg", "jpeg"];
    path.extension()
        .map(|x| EXTS.contains(&x.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn es_video(path: &Path) -> bool {
    const EXTS: [&str; 5] = ["mp4", "webm", "mkv", "mov", "gif"];
    path.extension()
        .map(|x| EXTS.contains(&x.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn listar_fondos(dir: &Path) -> Vec<PathBuf> {
    let mut archivos: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(e) => e
            .flatten()
            .map(|x| x.path())
            .filter(|p| extension_ok(p))
            .collect(),
        Err(_) => vec![],
    };
    archivos.sort();
    archivos
}

pub fn encontrar_fondo(dir: &Path) -> Option<PathBuf> {
    listar_fondos(dir).into_iter().find(|p| es_video(p))
}

/// Siguiente fondo en orden alfabético (cíclico) después de `actual`.
pub fn siguiente_fondo(dir: &Path, actual: Option<&str>) -> Option<PathBuf> {
    let fondos = listar_fondos(dir);
    if fondos.is_empty() {
        return None;
    }
    let idx = actual
        .and_then(|a| fondos.iter().position(|p| p.to_string_lossy() == a))
        .unwrap_or(fondos.len() - 1);
    Some(fondos[(idx + 1) % fondos.len()].clone())
}

/// Devuelve true si el equipo está funcionando con batería (no enchufado).
pub fn en_bateria() -> bool {
    let Ok(dir) = std::fs::read_dir("/sys/class/power_supply") else {
        return false;
    };
    for entrada in dir.flatten() {
        if let Ok(estado) = std::fs::read_to_string(entrada.path().join("status")) {
            if estado.trim() == "Discharging" {
                return true;
            }
        }
    }
    false
}

/// Envía un comando al daemon y devuelve la respuesta JSON.
pub fn ipc(cmd: &serde_json::Value) -> std::io::Result<serde_json::Value> {
    let mut stream = UnixStream::connect(socket_ipc())?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(cmd.to_string().as_bytes())?;
    stream.write_all(b"\n")?;

    let mut reader = BufReader::new(stream);
    let mut linea = String::new();
    reader.read_line(&mut linea)?;
    if linea.trim().is_empty() {
        return Ok(serde_json::json!({ "ok": true }));
    }
    serde_json::from_str(linea.trim())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Consulta el estado completo del daemon. None si no está en ejecución.
pub fn consultar_estado() -> Option<EstadoDaemon> {
    ipc(&serde_json::json!({ "cmd": "status" }))
        .ok()
        .and_then(|v| serde_json::from_value::<EstadoDaemon>(v).ok())
}

/// Envía un comando sin esperar respuesta.
pub fn enviar_ipc(cmd: &serde_json::Value) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(socket_ipc())?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(cmd.to_string().as_bytes())?;
    stream.write_all(b"\n")?;
    Ok(())
}

/// Nombre sin extensión de un archivo.
pub fn nombre_base(path: &Path) -> String {
    path.file_stem()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            path.file_name()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default()
        })
}

/// Peso de un archivo formateado para mostrar.
pub fn peso_de(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(m) => formato_peso(m.len()),
        Err(_) => "—".into(),
    }
}

pub fn formato_peso(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KB * KB * KB {
        format!("{:.1} GB", b / (KB * KB * KB))
    } else if b >= KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Clave única y estable para la caché de miniaturas de un fondo.
/// Incluye un hash de ruta canónica + mtime + tamaño para invalidarla
/// cuando el archivo cambia y evitar colisiones entre extensiones.
pub fn clave_cache(path: &Path) -> String {
    let stem: String = nombre_base(path)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match std::fs::canonicalize(path) {
        Ok(p) => p.hash(&mut h),
        Err(_) => path.hash(&mut h),
    }
    if let Ok(meta) = std::fs::metadata(path) {
        meta.len().hash(&mut h);
        if let Ok(mtime) = meta
            .modified()
            .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(std::io::Error::other))
        {
            mtime.as_secs().hash(&mut h);
        }
    }
    format!("{}_{:08x}", stem, h.finish() as u32)
}

/// Crea un `.desktop` de autostart apuntando a `exec` (o lo elimina).
pub fn aplicar_autostart(habilitado: bool, exec: &Path) {
    let ruta = dirs::config_dir()
        .unwrap_or_default()
        .join("autostart/wallpaper-animado.desktop");
    if habilitado {
        if let Some(padre) = ruta.parent() {
            let _ = std::fs::create_dir_all(padre);
        }
        let contenido = format!(
            "[Desktop Entry]\nType=Application\nName=Azura\nComment=Fondo de pantalla animado\nExec={}\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
            exec.display()
        );
        let _ = std::fs::write(&ruta, contenido);
    } else {
        let _ = std::fs::remove_file(&ruta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_temporal(nombre: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wallpaper-test-{}-{}", std::process::id(), nombre));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extensiones_reconocidas() {
        assert!(extension_ok(Path::new("/tmp/a.MP4")));
        assert!(extension_ok(Path::new("/tmp/a.jpeg")));
        assert!(!extension_ok(Path::new("/tmp/a.txt")));
        assert!(!extension_ok(Path::new("/tmp/a")));
    }

    #[test]
    fn videos_frente_a_imagenes() {
        assert!(es_video(Path::new("/tmp/a.webm")));
        assert!(!es_video(Path::new("/tmp/a.png")));
    }

    #[test]
    fn listar_fondos_filtra_y_ordena() {
        let dir = dir_temporal("listar");
        std::fs::write(dir.join("b.mp4"), b"x").unwrap();
        std::fs::write(dir.join("a.png"), b"x").unwrap();
        std::fs::write(dir.join("c.txt"), b"x").unwrap();
        let fondos = listar_fondos(&dir);
        let nombres: Vec<_> = fondos
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(nombres, vec!["a.png", "b.mp4"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn siguiente_fondo_es_ciclico() {
        let dir = dir_temporal("siguiente");
        std::fs::write(dir.join("a.mp4"), b"x").unwrap();
        std::fs::write(dir.join("b.mp4"), b"x").unwrap();
        let primero = dir.join("a.mp4");
        let segundo = dir.join("b.mp4");
        assert_eq!(
            siguiente_fondo(&dir, Some(primero.to_str().unwrap())),
            Some(segundo.clone())
        );
        assert_eq!(
            siguiente_fondo(&dir, Some(segundo.to_str().unwrap())),
            Some(primero)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn formato_peso_unidades() {
        assert_eq!(formato_peso(512), "512 B");
        assert_eq!(formato_peso(2048), "2 KB");
        assert_eq!(formato_peso(3 * 1024 * 1024), "3.0 MB");
        assert_eq!(formato_peso(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn config_ida_y_vuelta() {
        let dir = dir_temporal("config");
        let ruta = dir.join("config.json");
        let mut cfg = Config {
            current_wallpaper: Some("/tmp/video.mp4".into()),
            fps_limit: 30,
            hotkey: "super-w".into(),
            ..Default::default()
        };
        cfg.favoritos.push("/tmp/video.mp4".into());
        cfg.fondos_por_monitor
            .insert("HDMI-1".into(), "/tmp/otro.mp4".into());
        cfg.guardar_en(&ruta).unwrap();

        let leida = Config::cargar_de(&ruta);
        assert_eq!(leida, cfg);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_ignora_campos_desconocidos() {
        let dir = dir_temporal("config-futuro");
        let ruta = dir.join("config.json");
        std::fs::write(&ruta, r#"{"fps_limit": 24, "campo_futuro": true}"#).unwrap();
        let leida = Config::cargar_de(&ruta);
        assert_eq!(leida.fps_limit, 24);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clave_cache_cambia_con_el_contenido() {
        let dir = dir_temporal("cache");
        let archivo = dir.join("video.mp4");
        std::fs::write(&archivo, b"contenido-1").unwrap();
        let k1 = clave_cache(&archivo);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&archivo, b"contenido-2-mas-largo").unwrap();
        let k2 = clave_cache(&archivo);
        assert_ne!(k1, k2);
        assert!(k1.starts_with("video_"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hash_rapido_detecta_duplicados() {
        let dir = dir_temporal("hash");
        let a = dir.join("a.mp4");
        let b = dir.join("b.mp4");
        let c = dir.join("c.mp4");
        std::fs::write(&a, b"mismo contenido").unwrap();
        std::fs::write(&b, b"mismo contenido").unwrap();
        std::fs::write(&c, b"otro contenido distinto").unwrap();
        assert_eq!(hash_rapido(&a), hash_rapido(&b));
        assert_ne!(hash_rapido(&a), hash_rapido(&c));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn metadatos_parsean_ffprobe() {
        let json = r#"{
            "streams": [
                {"codec_type": "audio", "codec_name": "aac"},
                {"codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
                 "avg_frame_rate": "30/1", "r_frame_rate": "60/1", "duration": "45.7"}
            ],
            "format": {"duration": "46.0"}
        }"#;
        let meta = parsear_ffprobe(json).unwrap();
        assert_eq!(meta.ancho, 1920);
        assert_eq!(meta.alto, 1080);
        assert_eq!(meta.resolucion(), "1080p");
        assert_eq!(meta.fps, 30.0);
        assert_eq!(meta.duracion(), "0:46");
        assert_eq!(meta.codec, "h264");
    }

    #[test]
    fn metadatos_sin_stream_de_video() {
        let json = r#"{"streams": [{"codec_type": "audio"}], "format": {}}"#;
        assert!(parsear_ffprobe(json).is_none());
        assert!(parsear_ffprobe("no es json").is_none());
    }

    #[test]
    fn resolucion_no_estandar_y_fps_invalido() {
        let json = r#"{"streams": [{"codec_type": "video", "codec_name": "vp9", "width": 800,
            "height": 600, "avg_frame_rate": "0/0", "r_frame_rate": "24/1", "duration": "0"}]}"#;
        let meta = parsear_ffprobe(json).unwrap();
        assert_eq!(meta.resolucion(), "800x600");
        assert_eq!(meta.fps, 24.0);
        assert_eq!(meta.duracion(), "0:00");
    }

    #[test]
    fn slug_limpia_el_nombre() {
        assert_eq!(slug("Noche morada"), "noche-morada");
        assert_eq!(slug("  ¡Verano/Azul!!  "), "verano-azul");
        assert_eq!(slug("A--B"), "a-b");
        assert_eq!(slug("ñandú cálido"), "ñandú-cálido");
        assert_eq!(slug("!!!"), "perfil");
    }

    #[test]
    fn perfil_ida_y_vuelta_json() {
        let perfil = Perfil {
            nombre: "Noche morada".to_string(),
            gtk: Some("Mint-Y-Dark".to_string()),
            wm: Some("Mint-Y".to_string()),
            cinnamon: Some("Orchis-Dark".to_string()),
            fastfetch: Some("config.jsonc".to_string()),
            kitty: Some("kitty.conf".to_string()),
        };
        let texto = serde_json::to_string(&perfil).unwrap();
        let vuelta: Perfil = serde_json::from_str(&texto).unwrap();
        assert_eq!(vuelta.nombre, "Noche morada");
        assert_eq!(vuelta.gtk.as_deref(), Some("Mint-Y-Dark"));
        assert_eq!(vuelta.fastfetch.as_deref(), Some("config.jsonc"));
        assert_eq!(vuelta.kitty.as_deref(), Some("kitty.conf"));
        // Campos opcionales ausentes no rompen el parseo
        let minimo: Perfil = serde_json::from_str(r#"{"nombre": "Simple"}"#).unwrap();
        assert_eq!(minimo.gtk, None);
    }

    #[test]
    fn sockets_en_directorio_privado() {
        let dir = dir_sockets();
        assert!(dir.ends_with("wallpaper-animado"));
        assert!(socket_ipc().ends_with("wallpaper-animado/daemon.sock"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(modo, 0o700);
        }
    }

    #[test]
    fn eliminar_socket_solo_borra_sockets() {
        let dir = dir_temporal("sockets");

        // Un archivo normal no se toca.
        let archivo = dir.join("dato.txt");
        std::fs::write(&archivo, b"importante").unwrap();
        eliminar_socket(&archivo);
        assert!(archivo.exists());

        // Un symlink tampoco (aunque su destino sea un socket).
        let enlace = dir.join("enlace");
        std::os::unix::fs::symlink(&archivo, &enlace).unwrap();
        eliminar_socket(&enlace);
        assert!(std::fs::symlink_metadata(&enlace).is_ok());

        // Un socket sí se elimina.
        let sock = dir.join("daemon.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        eliminar_socket(&sock);
        assert!(!sock.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nombres_de_archivo_y_extensiones_de_perfil() {
        assert!(nombre_archivo_seguro("config.jsonc"));
        assert!(!nombre_archivo_seguro("../../.bashrc"));
        assert!(!nombre_archivo_seguro("sub/config.json"));
        assert!(!nombre_archivo_seguro("/etc/passwd"));
        assert!(!nombre_archivo_seguro(".."));
        assert!(!nombre_archivo_seguro(""));

        assert_eq!(extension_fastfetch("config.jsonc"), Some("jsonc"));
        assert_eq!(extension_fastfetch("config.json"), Some("json"));
        assert_eq!(extension_fastfetch("config.toml"), None);
        assert_eq!(extension_fastfetch("sin-extension"), None);
    }

    #[test]
    fn escritura_atomica_deja_el_archivo_completo() {
        let dir = dir_temporal("atomico");
        let ruta = dir.join("config.json");
        escribir_atomico(&ruta, r#"{"a":1}"#).unwrap();
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), r#"{"a":1}"#);

        // No deben quedar temporales tras un rename correcto.
        let residuos = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        assert_eq!(residuos, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
