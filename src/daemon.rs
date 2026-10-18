use anyhow::{Context, Result};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use ksni::menu::{CheckmarkItem, MenuItem as KsniMenuItem, StandardItem};
use ksni::ToolTip;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use wallpaper_core::{self as core, Config, EstadoDaemon};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::screensaver::ConnectionExt as _;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

static MANAGER_GUARD: std::sync::OnceLock<GlobalHotKeyManager> = std::sync::OnceLock::new();

/// Acción asociada a un atajo global registrado.
#[derive(Clone, Copy, PartialEq)]
enum Accion {
    Menu,
    Siguiente,
}

/// Geometría de un monitor detectado por RandR.
struct PantallaGeom {
    nombre: String,
    x: i16,
    y: i16,
    ancho: u16,
    alto: u16,
}

/// Una ventana de escritorio con su propio mpv.
struct Pantalla {
    nombre: String,
    win_id: u32,
    socket: PathBuf,
    mpv: Mutex<Option<Child>>,
    video_actual: Mutex<Option<PathBuf>>,
}

struct Estado {
    config: Mutex<Config>,
    pantallas: Vec<Pantalla>,
    dir: PathBuf,
    pausa_manual: AtomicBool,
    pantalla_bloqueada: Arc<AtomicBool>,
    hotkey_activo: Mutex<String>,
    hotkey_siguiente_activo: Mutex<String>,
    hotkeys: Mutex<Vec<(HotKey, Accion)>>,
    mpv_disponible: bool,
    ffmpeg_disponible: bool,
    inicio: Instant,
    _bandeja: Mutex<Option<ksni::blocking::Handle<Bandeja>>>,
    _lock: std::fs::File,
}

fn main() -> Result<()> {
    env_logger::init();
    log::info!("🚀 wallpaper-daemon iniciando");

    let lock = bloquear_instancia().context("instancia duplicada")?;

    let mpv_disponible = comando_disponible("mpv");
    let ffmpeg_disponible = comando_disponible("ffmpeg");
    if !mpv_disponible {
        log::error!("❌ mpv no está instalado o no está en PATH; el fondo no se reproducirá");
    }
    if !ffmpeg_disponible {
        log::warn!("⚠️  ffmpeg no está instalado; no se generarán miniaturas");
    }

    let dir = core::wallpapers_dir();
    let mut config = Config::cargar();

    // Fondo inicial: el de la config si existe, si no el primero disponible
    let wallpaper = config
        .current_wallpaper
        .as_ref()
        .map(PathBuf::from)
        .filter(|p| p.exists() && core::extension_ok(p))
        .or_else(|| core::encontrar_fondo(&dir))
        .context("no hay fondos disponibles")?;
    config.current_wallpaper = Some(wallpaper.to_string_lossy().into_owned());
    if let Err(e) = config.guardar() {
        log::warn!("⚠️  No se pudo guardar la configuración: {}", e);
    }
    log::info!("🎬 Fondo inicial: {:?}", wallpaper);

    if config.autostart {
        if let Ok(exe) = std::env::current_exe() {
            core::aplicar_autostart(true, &exe);
        }
    }

    // 1. Ventanas X11 (una por monitor) en el escritorio
    let (conn, screen_num) = RustConnection::connect(None).context("no se pudo conectar a X11")?;
    let root = conn.setup().roots[screen_num].root;
    let geometrias = monitores(&conn, root);
    let mut pantallas = Vec::new();
    for (i, geom) in geometrias.iter().enumerate() {
        let win_id =
            crear_ventana_escritorio(&conn, screen_num, geom.x, geom.y, geom.ancho, geom.alto)?;
        log::info!("🪟 Monitor «{}» → ventana 0x{:x}", geom.nombre, win_id);
        pantallas.push(Pantalla {
            nombre: geom.nombre.clone(),
            win_id,
            socket: socket_mpv(i),
            mpv: Mutex::new(None),
            video_actual: Mutex::new(None),
        });
    }

    // 2. Lanzar mpv dentro de cada ventana
    for p in &pantallas {
        if let Err(e) = relanzar_pantalla_mpv(&config, p, false) {
            log::error!("🎥 mpv en «{}»: {}", p.nombre, e);
        }
    }
    thread::sleep(Duration::from_millis(1500));

    let estado = Arc::new(Estado {
        config: Mutex::new(config),
        pantallas,
        dir: dir.clone(),
        pausa_manual: AtomicBool::new(false),
        pantalla_bloqueada: Arc::new(AtomicBool::new(false)),
        hotkey_activo: Mutex::new("Ninguno".into()),
        hotkey_siguiente_activo: Mutex::new("Ninguno".into()),
        hotkeys: Mutex::new(Vec::new()),
        mpv_disponible,
        ffmpeg_disponible,
        inicio: Instant::now(),
        _bandeja: Mutex::new(None),
        _lock: lock,
    });

    // 3. Bandeja del sistema (si el escritorio soporta StatusNotifierItem)
    match ksni::blocking::TrayMethods::spawn(Bandeja {
        estado: estado.clone(),
    }) {
        Ok(handle) => {
            *estado._bandeja.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
            log::info!("📌 Bandeja registrada");
        }
        Err(e) => log::warn!("📌 Sin bandeja del sistema: {}", e),
    }

    // 4. Watcher (pausas automáticas + cambios de resolución)
    {
        let estado = estado.clone();
        let (conn_watch, _) = RustConnection::connect(None).context("watcher: conectar X11")?;
        thread::spawn(move || {
            if let Err(e) = vigilar_ventanas(conn_watch, estado) {
                log::error!("watcher: {}", e);
            }
        });
    }

    // 5. Slideshow (cambio automático de fondo)
    {
        let estado = estado.clone();
        thread::spawn(move || {
            if let Err(e) = slideshow_loop(estado) {
                log::error!("slideshow: {}", e);
            }
        });
    }

    // 6. Vigilante de mpv (relanza si se cae)
    {
        let estado = estado.clone();
        thread::spawn(move || vigilar_mpv(estado));
    }

    // 7. Atajos globales (menú y siguiente fondo)
    registrar_hotkeys(&estado);

    // 7b. Estado de bloqueo de pantalla (DBus)
    {
        let estado = estado.clone();
        thread::spawn(move || vigilar_bloqueo(estado));
    }

    // 8. Servidor IPC (el menú y la bandeja envían comandos aquí)
    let socket_ipc = core::socket_ipc();
    core::eliminar_socket(&socket_ipc);
    let listener = UnixListener::bind(&socket_ipc)?;
    let _ = std::fs::set_permissions(&socket_ipc, std::fs::Permissions::from_mode(0o600));
    log::info!("🔌 IPC en {}", socket_ipc.display());

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Err(e) = manejar_cliente(s, &estado) {
                    log::error!("cliente: {}", e);
                }
            }
            Err(e) => log::error!("accept: {}", e),
        }
    }
    Ok(())
}

// ── Instancia única y dependencias ─────────────────────────────

/// Bloquea un flock exclusivo; si ya hay otra instancia, falla.
fn bloquear_instancia() -> Result<std::fs::File> {
    let ruta = dirs_cache().join("wallpaper-animado").join("daemon.lock");
    if let Some(padre) = ruta.parent() {
        std::fs::create_dir_all(padre)?;
    }
    let archivo = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&ruta)?;
    let rc = unsafe { libc::flock(archivo.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        anyhow::bail!("ya hay otra instancia del daemon en ejecución");
    }
    Ok(archivo)
}

fn dirs_cache() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn comando_disponible(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

// ── Monitores y ventanas ───────────────────────────────────────

/// Detecta los monitores con RandR 1.5; si no está disponible, una sola pantalla.
fn monitores(conn: &RustConnection, root: u32) -> Vec<PantallaGeom> {
    if let Ok(cookie) = conn.randr_get_monitors(root, true) {
        if let Ok(reply) = cookie.reply() {
            let lista: Vec<PantallaGeom> = reply
                .monitors
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    // El nombre llega como átomo; se resuelve a texto
                    let nombre = conn
                        .get_atom_name(m.name)
                        .ok()
                        .and_then(|c| c.reply().ok())
                        .map(|r| String::from_utf8_lossy(&r.name).to_string())
                        .unwrap_or_else(|| format!("Monitor {}", i + 1));
                    PantallaGeom {
                        nombre,
                        x: m.x,
                        y: m.y,
                        ancho: m.width,
                        alto: m.height,
                    }
                })
                .collect();
            if !lista.is_empty() {
                return lista;
            }
        }
    }
    let screen = &conn.setup().roots[0];
    vec![PantallaGeom {
        nombre: "Principal".into(),
        x: 0,
        y: 0,
        ancho: screen.width_in_pixels,
        alto: screen.height_in_pixels,
    }]
}

fn crear_ventana_escritorio(
    conn: &RustConnection,
    screen_num: usize,
    x: i16,
    y: i16,
    ancho: u16,
    alto: u16,
) -> Result<u32> {
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // Depth 24 = RGB estándar. Compatible con --vo=gpu de mpv, sin colormap especial.
    let depth = 24;
    let visual = screen.root_visual;

    let win = conn.generate_id()?;
    conn.create_window(
        depth,
        win,
        root,
        x,
        y,
        ancho,
        alto,
        0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &CreateWindowAux::new()
            .background_pixel(screen.black_pixel)
            .border_pixel(0)
            .event_mask(EventMask::EXPOSURE),
    )?;

    // Marcar como ventana de escritorio (para que no la gestione el WM)
    let wm_type = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
        .reply()?
        .atom;
    let wm_type_desktop = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE_DESKTOP")?
        .reply()?
        .atom;
    conn.change_property32(
        PropMode::REPLACE,
        win,
        wm_type,
        AtomEnum::ATOM,
        &[wm_type_desktop],
    )?;

    // Estado: below + sticky (siempre abajo, en todos los escritorios)
    let wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    let st_below = conn
        .intern_atom(false, b"_NET_WM_STATE_BELOW")?
        .reply()?
        .atom;
    let st_sticky = conn
        .intern_atom(false, b"_NET_WM_STATE_STICKY")?
        .reply()?
        .atom;
    conn.change_property32(
        PropMode::REPLACE,
        win,
        wm_state,
        AtomEnum::ATOM,
        &[st_below, st_sticky],
    )?;

    conn.map_window(win)?;
    conn.configure_window(win, &ConfigureWindowAux::new().stack_mode(StackMode::BELOW))?;
    conn.flush()?;

    Ok(win)
}

/// Reposiciona las ventanas según la disposición actual de monitores.
fn relayout(conn: &RustConnection, estado: &Estado) {
    let root = conn.setup().roots[0].root;
    let geoms = monitores(conn, root);
    for (i, p) in estado.pantallas.iter().enumerate() {
        let geom = geoms
            .iter()
            .find(|g| g.nombre == p.nombre)
            .or_else(|| geoms.get(i));
        if let Some(g) = geom {
            let _ = conn.configure_window(
                p.win_id,
                &ConfigureWindowAux::new()
                    .x(g.x as i32)
                    .y(g.y as i32)
                    .width(g.ancho as u32)
                    .height(g.alto as u32)
                    .stack_mode(StackMode::BELOW),
            );
        }
    }
    let _ = conn.flush();
    log::info!("🖥️  Disposición de monitores actualizada");
}

// ── mpv ────────────────────────────────────────────────────────

fn socket_mpv(indice: usize) -> PathBuf {
    core::dir_sockets().join(format!("mpv-{}.sock", indice))
}

fn enviar_mpv(socket: &Path, cmd: &str) -> Result<()> {
    let mut stream = UnixStream::connect(socket).context("no se pudo conectar al socket de mpv")?;
    stream.write_all(cmd.as_bytes())?;
    stream.write_all(b"\n")?;
    Ok(())
}

fn enviar_a_todas(estado: &Estado, cmd: &str) {
    for p in &estado.pantallas {
        let _ = enviar_mpv(&p.socket, cmd);
    }
}

fn pausa_mpv(pausar: bool) -> &'static str {
    if pausar {
        r#"{"command":["set_property","pause",true]}"#
    } else {
        r#"{"command":["set_property","pause",false]}"#
    }
}

fn cargar_video(socket: &Path, video: &Path) {
    let json = serde_json::json!({
        "command": ["loadfile", video.to_string_lossy(), "replace"]
    });
    let _ = enviar_mpv(socket, &json.to_string());
}

/// Carga un vídeo en la pantalla y recuerda cuál está reproduciendo.
fn poner_video(p: &Pantalla, video: &Path) {
    cargar_video(&p.socket, video);
    *p.video_actual.lock().unwrap_or_else(|e| e.into_inner()) = Some(video.to_path_buf());
}

/// Fondo que corresponde a una pantalla: su override o el global.
fn fondo_de_pantalla(cfg: &Config, nombre: &str) -> Option<PathBuf> {
    cfg.fondos_por_monitor
        .get(nombre)
        .map(PathBuf::from)
        .or_else(|| cfg.current_wallpaper.as_ref().map(PathBuf::from))
}

fn matar_mpv(p: &Pantalla) {
    if let Some(mut hijo) = p.mpv.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = hijo.kill();
        let _ = hijo.wait();
    }
}

/// Lanza el mpv de una pantalla con el fondo que le corresponda.
fn relanzar_pantalla_mpv(cfg: &Config, p: &Pantalla, pausado: bool) -> Result<()> {
    let video = fondo_de_pantalla(cfg, &p.nombre).context("sin fondo para la pantalla")?;
    matar_mpv(p);
    core::eliminar_socket(&p.socket);
    let child = lanzar_mpv(p.win_id, &p.socket, &video, cfg.fps_limit)?;
    log::info!("🎥 mpv «{}» PID {} → {:?}", p.nombre, child.id(), video);
    *p.mpv.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
    *p.video_actual.lock().unwrap_or_else(|e| e.into_inner()) = Some(video.clone());
    if pausado {
        thread::sleep(Duration::from_millis(300));
        let _ = enviar_mpv(&p.socket, pausa_mpv(true));
    }
    Ok(())
}

fn lanzar_mpv(win_id: u32, socket: &Path, video: &Path, fps_limit: u32) -> Result<Child> {
    let mut cmd = Command::new("mpv");
    cmd.args([
        &format!("--wid={}", win_id),
        &format!("--input-ipc-server={}", socket.display()),
        "--no-audio",
        "--loop-file=inf",
        "--no-osc",
        "--no-input-default-bindings",
        "--no-terminal",
        "--vo=gpu",
        "--hwdec=auto-safe",
        "--cache=yes",
        "--demuxer-max-bytes=30M",
        "--really-quiet",
    ]);
    if fps_limit > 0 {
        cmd.arg(format!("--fps={}", fps_limit));
    }
    cmd.arg(video)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("no se pudo lanzar mpv")
}

// ── Cambio de fondos ───────────────────────────────────────────

/// Aplica un fondo a todas las pantallas y borra los overrides por monitor.
fn aplicar_global(estado: &Estado, video: &Path) {
    let pausado = estado.pausa_manual.load(Ordering::Relaxed);
    {
        let mut cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
        cfg.current_wallpaper = Some(video.to_string_lossy().into_owned());
        cfg.fondos_por_monitor.clear();
        if let Err(e) = cfg.guardar() {
            log::warn!("no se pudo guardar la configuración: {}", e);
        }
    }
    for p in &estado.pantallas {
        if p.mpv.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            let cfg = estado
                .config
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            let _ = relanzar_pantalla_mpv(&cfg, p, pausado);
        } else {
            poner_video(p, video);
        }
    }
    if !pausado {
        enviar_a_todas(estado, pausa_mpv(false));
    }
}

/// Aplica un fondo solo a un monitor (queda fijado como override).
fn aplicar_monitor(estado: &Estado, monitor: &str, video: &Path) -> Result<()> {
    let pantalla = estado
        .pantallas
        .iter()
        .find(|p| p.nombre == monitor)
        .with_context(|| format!("monitor desconocido: {}", monitor))?;
    let pausado = estado.pausa_manual.load(Ordering::Relaxed);
    {
        let mut cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
        cfg.fondos_por_monitor
            .insert(monitor.to_string(), video.to_string_lossy().into_owned());
        if let Err(e) = cfg.guardar() {
            log::warn!("no se pudo guardar la configuración: {}", e);
        }
    }
    if pantalla
        .mpv
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_none()
    {
        let cfg = estado
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        relanzar_pantalla_mpv(&cfg, pantalla, pausado)?;
    } else {
        poner_video(pantalla, video);
    }
    if !pausado {
        let _ = enviar_mpv(&pantalla.socket, pausa_mpv(false));
    }
    Ok(())
}

/// Cambia el fondo global sin tocar los monitores que tienen override (slideshow).
fn aplicar_automatico(estado: &Estado, video: &Path) {
    let pausado = estado.pausa_manual.load(Ordering::Relaxed);
    {
        let mut cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
        cfg.current_wallpaper = Some(video.to_string_lossy().into_owned());
        if let Err(e) = cfg.guardar() {
            log::warn!("no se pudo guardar la configuración: {}", e);
        }
    }
    for p in &estado.pantallas {
        let con_override = {
            let cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
            cfg.fondos_por_monitor.contains_key(&p.nombre)
        };
        if con_override {
            continue;
        }
        if p.mpv.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            let cfg = estado
                .config
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            let _ = relanzar_pantalla_mpv(&cfg, p, pausado);
        } else {
            poner_video(p, video);
        }
    }
    if !pausado {
        let cmd = pausa_mpv(false);
        for p in &estado.pantallas {
            let con_override = {
                let cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
                cfg.fondos_por_monitor.contains_key(&p.nombre)
            };
            if !con_override {
                let _ = enviar_mpv(&p.socket, cmd);
            }
        }
    }
}

/// Detiene todos los mpv y deja las pantallas vacías.
fn quitar_fondo(estado: &Estado) {
    for p in &estado.pantallas {
        matar_mpv(p);
        *p.video_actual.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    let mut cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.current_wallpaper = None;
    cfg.fondos_por_monitor.clear();
    if let Err(e) = cfg.guardar() {
        log::warn!("no se pudo guardar la configuración: {}", e);
    }
}

fn alternar_pausa(estado: &Estado, pausar: bool) {
    estado.pausa_manual.store(pausar, Ordering::Relaxed);
    let cmd = pausa_mpv(pausar);
    enviar_a_todas(estado, cmd);
    log::info!("{} pausa manual", if pausar { "⏸️ " } else { "▶️ " });
}

/// Reconciliación tras editar la config desde el menú: cada pantalla debe
/// reproducir exactamente su fondo esperado (override por monitor o global).
fn sincronizar_pantallas(estado: &Estado, cfg: &Config) {
    let pausado = estado.pausa_manual.load(Ordering::Relaxed);
    for p in &estado.pantallas {
        let esperado = fondo_de_pantalla(cfg, &p.nombre);
        let actual = p
            .video_actual
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        match esperado {
            Some(video) if actual.as_deref() != Some(video.as_path()) => {
                if p.mpv.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
                    let _ = relanzar_pantalla_mpv(cfg, p, pausado);
                } else {
                    poner_video(p, &video);
                    if !pausado {
                        let _ = enviar_mpv(&p.socket, pausa_mpv(false));
                    }
                }
            }
            None if actual.is_some()
                || p.mpv.lock().unwrap_or_else(|e| e.into_inner()).is_some() =>
            {
                if let Some(mut hijo) = p.mpv.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                }
                *p.video_actual.lock().unwrap_or_else(|e| e.into_inner()) = None;
            }
            _ => {}
        }
    }
}

fn fondo_aleatorio(video_actual: Option<&str>, fondos: &[PathBuf]) -> Option<PathBuf> {
    let candidatos: Vec<&PathBuf> = fondos
        .iter()
        .filter(|p| {
            video_actual
                .map(|a| p.to_string_lossy() != a)
                .unwrap_or(true)
        })
        .collect();
    if candidatos.is_empty() {
        return fondos.first().cloned();
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as usize;
    Some(candidatos[nanos % candidatos.len()].clone())
}

/// Pasa al siguiente fondo (con notificación opcional).
fn avanzar_siguiente(estado: &Estado, notificar: bool) {
    let actual = estado
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .current_wallpaper
        .clone();
    if let Some(video) = core::siguiente_fondo(&estado.dir, actual.as_deref()) {
        aplicar_global(estado, &video);
        if notificar {
            notificar_fondo(&video);
        }
    }
}

/// Aplica un fondo al azar (con notificación opcional).
fn avanzar_aleatorio(estado: &Estado, notificar: bool) {
    let actual = estado
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .current_wallpaper
        .clone();
    let fondos = core::listar_fondos(&estado.dir);
    if let Some(video) = fondo_aleatorio(actual.as_deref(), &fondos) {
        aplicar_global(estado, &video);
        if notificar {
            notificar_fondo(&video);
        }
    }
}

/// Avisa del cambio de fondo con una notificación de escritorio.
fn notificar_fondo(video: &Path) {
    let _ = Command::new("notify-send")
        .args([
            "-a",
            "Azura",
            "-i",
            "wallpaper-animado",
            "Fondo cambiado",
            &core::nombre_base(video),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

// ── Bloqueo de pantalla (DBus) ─────────────────────────────────

/// Escucha las señales ActiveChanged de los screensavers habituales y marca
/// el estado de bloqueo para que el watcher pause el fondo.
fn vigilar_bloqueo(estado: Arc<Estado>) {
    let conn = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("🔒 Sin DBus de sesión; no se pausará al bloquear: {}", e);
            return;
        }
    };

    // (servicio, ruta) de los screensavers más comunes
    let objetivos: [(&str, &str); 4] = [
        (
            "org.freedesktop.ScreenSaver",
            "/org/freedesktop/ScreenSaver",
        ),
        ("org.freedesktop.ScreenSaver", "/ScreenSaver"),
        ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),
        ("org.cinnamon.ScreenSaver", "/org/cinnamon/ScreenSaver"),
    ];

    for (servicio, ruta) in objetivos {
        let conn2 = conn.clone();
        let flag = estado.pantalla_bloqueada.clone();
        thread::spawn(move || {
            let Ok(proxy) = zbus::blocking::Proxy::new(&conn2, servicio, ruta, servicio) else {
                return;
            };
            let Ok(senales) = proxy.receive_signal("ActiveChanged") else {
                return;
            };
            for msg in senales {
                if let Ok((activo,)) = msg.body().deserialize::<(bool,)>() {
                    flag.store(activo, Ordering::Relaxed);
                    log::info!(
                        "{} pantalla {}",
                        if activo { "🔒" } else { "🔓" },
                        if activo { "bloqueada" } else { "desbloqueada" }
                    );
                }
            }
        });
    }
}

// ── Watcher de pausas ──────────────────────────────────────────

fn leer_prop_u32(
    conn: &RustConnection,
    win: u32,
    atom: u32,
    tipo: AtomEnum,
    largo: u32,
) -> Vec<u32> {
    conn.get_property(false, win, atom, tipo, 0, largo)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().map(|it| it.collect()))
        .unwrap_or_default()
}

fn leer_desktop(conn: &RustConnection, root: u32, atom: u32) -> u32 {
    leer_prop_u32(conn, root, atom, AtomEnum::CARDINAL, 1)
        .first()
        .copied()
        .unwrap_or(0)
}

fn vigilar_ventanas(conn: RustConnection, estado: Arc<Estado>) -> Result<()> {
    let root = conn.setup().roots[0].root;

    // Recibir ConfigureNotify del root para detectar cambios de resolución
    conn.change_window_attributes(
        root,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
    )?;
    conn.flush()?;

    let active_atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
        .reply()?
        .atom;
    let wm_state_atom = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    let fullscreen_atom = conn
        .intern_atom(false, b"_NET_WM_STATE_FULLSCREEN")?
        .reply()?
        .atom;
    let wm_type_atom = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
        .reply()?
        .atom;
    let wt_normal = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE_NORMAL")?
        .reply()?
        .atom;
    let current_desktop_atom = conn
        .intern_atom(false, b"_NET_CURRENT_DESKTOP")?
        .reply()?
        .atom;

    // Desktop actual al arrancar (para detectar cambios posteriores)
    let mut desktop_anterior: Option<u32> = Some(leer_desktop(&conn, root, current_desktop_atom));
    // Instante hasta el que hay que forzar la pausa tras cambiar de workspace
    let mut pausa_workspace_hasta: Option<Instant> = None;

    let mut pausado = false;
    loop {
        // Drenar eventos X11 pendientes (resolución / disposición de monitores)
        loop {
            match conn.poll_for_event() {
                Ok(Some(x11rb::protocol::Event::ConfigureNotify(e))) => {
                    if e.window == root {
                        relayout(&conn, &estado);
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => {
                    log::warn!("watcher: evento X11: {}", e);
                    break;
                }
            }
        }

        let config = estado
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        // 1. Ventana con foco de entrada (_NET_ACTIVE_WINDOW)
        let activa: u32 = leer_prop_u32(&conn, root, active_atom, AtomEnum::WINDOW, 1)
            .first()
            .copied()
            .unwrap_or(0);
        let es_nuestra = estado.pantallas.iter().any(|p| p.win_id == activa);

        // 2. Cambio de espacio de trabajo → forzar pausa unos segundos
        if config.pausa_workspace {
            let desktop = leer_desktop(&conn, root, current_desktop_atom);
            if let Some(previo) = desktop_anterior {
                if previo != desktop {
                    log::info!("🔀 workspace {} → {}, pausando un momento", previo, desktop);
                    pausa_workspace_hasta = Some(Instant::now() + Duration::from_millis(1200));
                }
            }
            desktop_anterior = Some(desktop);
        }

        // 3. Decidir pausa según la configuración y la pausa manual
        let mut debe_pausar = estado.pausa_manual.load(Ordering::Relaxed);

        if config.pausa_bateria && core::en_bateria() {
            debe_pausar = true;
        }

        // Pausar mientras una aplicación tiene el foco (no una ventana nuestra)
        if config.pausa_app && activa != 0 && !es_nuestra {
            debe_pausar = true;
        }

        // Pausar con una app normal en pantalla completa con foco
        if config.pausa_fullscreen && activa != 0 && !es_nuestra {
            let wt = leer_prop_u32(&conn, activa, wm_type_atom, AtomEnum::ATOM, 1);
            let es_fullscreen = leer_prop_u32(&conn, activa, wm_state_atom, AtomEnum::ATOM, 16)
                .contains(&fullscreen_atom);
            if wt.first().copied().unwrap_or(0) == wt_normal && es_fullscreen {
                debe_pausar = true;
            }
        }

        // Pausa temporal tras cambiar de workspace
        if let Some(hasta) = pausa_workspace_hasta {
            if Instant::now() < hasta {
                debe_pausar = true;
            } else {
                pausa_workspace_hasta = None;
            }
        }

        // Pausar con la pantalla bloqueada
        if config.pausa_bloqueo && estado.pantalla_bloqueada.load(Ordering::Relaxed) {
            debe_pausar = true;
        }

        // Pausar tras un tiempo sin actividad del usuario
        if config.pausa_inactividad {
            let limite = config.pausa_inactividad_minutos.max(1) * 60;
            if let Ok(cookie) = conn.screensaver_query_info(root) {
                if let Ok(info) = cookie.reply() {
                    if info.ms_since_user_input / 1000 >= limite {
                        debe_pausar = true;
                    }
                }
            }
        }

        if debe_pausar != pausado {
            pausado = debe_pausar;
            if pausado {
                log::info!("⏸️  pausando");
            } else {
                log::info!("▶️  reanudando");
            }
            enviar_a_todas(&estado, pausa_mpv(pausado));
        }
        thread::sleep(Duration::from_millis(700));
    }
}

// ── Vigilancia de mpv ──────────────────────────────────────────

fn vigilar_mpv(estado: Arc<Estado>) {
    loop {
        thread::sleep(Duration::from_secs(2));
        if !estado.mpv_disponible {
            continue;
        }
        for p in &estado.pantallas {
            let muerto = {
                let mut guard = p.mpv.lock().unwrap_or_else(|e| e.into_inner());
                match guard.as_mut() {
                    Some(hijo) => match hijo.try_wait() {
                        Ok(Some(estado_salida)) => {
                            log::warn!(
                                "🎥 mpv «{}» terminó ({:?}); relanzando",
                                p.nombre,
                                estado_salida
                            );
                            *guard = None;
                            true
                        }
                        Ok(None) => false,
                        Err(e) => {
                            log::warn!("🎥 mpv «{}»: {}", p.nombre, e);
                            *guard = None;
                            true
                        }
                    },
                    None => false,
                }
            };
            if muerto {
                let (cfg, pausado) = {
                    let cfg = estado
                        .config
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    let pausado = estado.pausa_manual.load(Ordering::Relaxed);
                    (cfg, pausado)
                };
                if fondo_de_pantalla(&cfg, &p.nombre).is_some() {
                    if let Err(e) = relanzar_pantalla_mpv(&cfg, p, pausado) {
                        log::error!("🎥 no se pudo relanzar mpv «{}»: {}", p.nombre, e);
                    }
                }
            }
        }
    }
}

// ── Slideshow ──────────────────────────────────────────────────

fn slideshow_loop(estado: Arc<Estado>) -> Result<()> {
    let mut ultimo_cambio = Instant::now();
    loop {
        thread::sleep(Duration::from_secs(10));

        let (habilitado, intervalo_min, aleatorio, actual) = {
            let cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
            (
                cfg.slideshow,
                cfg.slideshow_minutos.max(1) as u64,
                cfg.slideshow_aleatorio,
                cfg.current_wallpaper.clone(),
            )
        };

        if !habilitado {
            // Reiniciar el temporizador para que al activarlo no cambie al instante
            ultimo_cambio = Instant::now();
            continue;
        }

        if ultimo_cambio.elapsed() < Duration::from_secs(intervalo_min * 60) {
            continue;
        }

        let fondos = core::listar_fondos(&estado.dir);
        if fondos.is_empty() {
            continue;
        }

        let elegido = if aleatorio {
            fondo_aleatorio(actual.as_deref(), &fondos)
        } else {
            core::siguiente_fondo(&estado.dir, actual.as_deref())
        };

        if let Some(video) = elegido {
            log::info!("🔄 Slideshow: cambiando a {:?}", video);
            aplicar_automatico(&estado, &video);
            notificar_fondo(&video);
        }
        ultimo_cambio = Instant::now();
    }
}

// ── Atajo global ───────────────────────────────────────────────

fn preset_hotkey(id: &str) -> Option<HotKey> {
    match id {
        "ctrl-shift-w" => Some(HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Code::KeyW,
        )),
        "ctrl-alt-w" => Some(HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::ALT),
            Code::KeyW,
        )),
        "super-w" => Some(HotKey::new(Some(Modifiers::SUPER), Code::KeyW)),
        "ctrl-alt-f12" => Some(HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::ALT),
            Code::F12,
        )),
        _ => None,
    }
}

/// Intenta registrar el atajo preferido; si está ocupado, prueba el resto de presets.
fn intentar_registrar(manager: &GlobalHotKeyManager, preferido: &str) -> Option<(HotKey, String)> {
    if preferido == "ninguno" {
        return None;
    }
    let mut orden: Vec<&str> = vec![preferido];
    for (id, _) in core::HOTKEYS {
        if id != preferido && id != "ninguno" {
            orden.push(id);
        }
    }
    for id in orden {
        if let Some(hk) = preset_hotkey(id) {
            match manager.register(hk) {
                Ok(_) => return Some((hk, core::hotkey_descripcion(id))),
                Err(e) => log::warn!(
                    "⚠️  No se pudo registrar {}: {}",
                    core::hotkey_descripcion(id),
                    e
                ),
            }
        }
    }
    None
}

/// Registra los dos atajos globales (menú y siguiente fondo) y escucha los eventos.
fn registrar_hotkeys(estado: &Arc<Estado>) {
    let Ok(manager) = GlobalHotKeyManager::new() else {
        log::warn!("⌨️  No se pudo crear GlobalHotKeyManager. Sin atajos.");
        return;
    };

    let (preferido_menu, preferido_siguiente) = {
        let cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
        (cfg.hotkey.clone(), cfg.hotkey_siguiente.clone())
    };

    let mut registrados = Vec::new();
    if let Some((hk, desc)) = intentar_registrar(&manager, &preferido_menu) {
        log::info!("⌨️  Atajo menú: {}", desc);
        *estado
            .hotkey_activo
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = desc;
        registrados.push((hk, Accion::Menu));
    } else {
        *estado
            .hotkey_activo
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = "Ninguno".into();
        log::warn!("⚠️  Sin atajo para abrir el menú");
    }
    if let Some((hk, desc)) = intentar_registrar(&manager, &preferido_siguiente) {
        log::info!("⌨️  Atajo siguiente: {}", desc);
        *estado
            .hotkey_siguiente_activo
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = desc;
        registrados.push((hk, Accion::Siguiente));
    } else {
        *estado
            .hotkey_siguiente_activo
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = "Ninguno".into();
    }
    *estado.hotkeys.lock().unwrap_or_else(|e| e.into_inner()) = registrados;

    let _ = MANAGER_GUARD.set(manager);

    let estado2 = estado.clone();
    thread::spawn(move || {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.recv() {
                if event.state != HotKeyState::Pressed {
                    continue;
                }
                let accion = estado2
                    .hotkeys
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .find(|(hk, _)| hk.id() == event.id)
                    .map(|(_, a)| *a);
                match accion {
                    Some(Accion::Menu) => {
                        log::info!("🔥 Atajo: abrir menú");
                        lanzar_menu(&estado2.dir);
                    }
                    Some(Accion::Siguiente) => {
                        log::info!("🔥 Atajo: siguiente fondo");
                        avanzar_siguiente(&estado2, true);
                    }
                    None => {}
                }
            }
        }
    });
}

/// Cambia los atajos activos al guardar la config (si cambiaron).
fn reregistrar_hotkeys(estado: &Estado, menu_id: &str, siguiente_id: &str) {
    let Some(manager) = MANAGER_GUARD.get() else {
        return;
    };
    {
        let mut guard = estado.hotkeys.lock().unwrap_or_else(|e| e.into_inner());
        for (hk, _) in guard.drain(..) {
            let _ = manager.unregister(hk);
        }
    }

    let mut registrados = Vec::new();
    if let Some((hk, desc)) = intentar_registrar(manager, menu_id) {
        *estado
            .hotkey_activo
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = desc.clone();
        log::info!("⌨️  Atajo menú cambiado a: {}", desc);
        registrados.push((hk, Accion::Menu));
    } else {
        *estado
            .hotkey_activo
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = "Ninguno".into();
    }
    if let Some((hk, desc)) = intentar_registrar(manager, siguiente_id) {
        *estado
            .hotkey_siguiente_activo
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = desc.clone();
        log::info!("⌨️  Atajo siguiente cambiado a: {}", desc);
        registrados.push((hk, Accion::Siguiente));
    } else {
        *estado
            .hotkey_siguiente_activo
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = "Ninguno".into();
    }
    *estado.hotkeys.lock().unwrap_or_else(|e| e.into_inner()) = registrados;
}

// ── IPC ────────────────────────────────────────────────────────

/// Tamaño máximo de un mensaje IPC (la petición es una única línea JSON).
const MAX_MENSAJE_IPC: u64 = 64 * 1024;

/// UID del proceso conectado al otro lado del socket (SO_PEERCRED).
fn uid_del_cliente(stream: &UnixStream) -> std::io::Result<libc::uid_t> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut longitud = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut libc::ucred as *mut libc::c_void,
            &mut longitud,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(cred.uid)
}

#[derive(serde::Deserialize)]
struct Comando {
    cmd: String,
    path: Option<String>,
    monitor: Option<String>,
    config: Option<Config>,
}

fn manejar_cliente(stream: UnixStream, estado: &Arc<Estado>) -> Result<()> {
    // Sin timeout, un cliente que conecte y nunca envíe nada bloquearía el IPC completo.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));

    // Solo aceptamos clientes del mismo usuario que ejecuta el daemon.
    match uid_del_cliente(&stream) {
        Ok(uid) if uid == unsafe { libc::getuid() } => {}
        Ok(_) => anyhow::bail!("cliente de otro usuario rechazado"),
        Err(e) => anyhow::bail!("no se pudo verificar el cliente: {}", e),
    }

    let mut escritor = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let mut buf = String::new();
    // Límite de tamaño: sin él, un cliente podría agotar la memoria del daemon.
    (&mut reader).take(MAX_MENSAJE_IPC).read_line(&mut buf)?;
    if buf.len() as u64 >= MAX_MENSAJE_IPC && !buf.ends_with('\n') {
        anyhow::bail!("mensaje IPC demasiado largo");
    }
    log::debug!("📨 IPC: {}", buf.trim());

    let (respuesta, salir) = match serde_json::from_str::<Comando>(&buf) {
        Ok(cmd) => {
            let salir = cmd.cmd == "quit";
            (ejecutar(cmd, estado), salir)
        }
        Err(e) => (
            serde_json::json!({ "ok": false, "error": format!("comando inválido: {}", e) }),
            false,
        ),
    };

    escritor.write_all(respuesta.to_string().as_bytes())?;
    escritor.write_all(b"\n")?;

    if salir {
        log::info!("👋 Saliendo por petición");
        limpiar_salida(estado);
        std::process::exit(0);
    }
    Ok(())
}

fn ejecutar(cmd: Comando, estado: &Arc<Estado>) -> serde_json::Value {
    match cmd.cmd.as_str() {
        "ping" => serde_json::json!({ "ok": true }),
        "status" => {
            let cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
            let estado_daemon = EstadoDaemon {
                version: env!("CARGO_PKG_VERSION").to_string(),
                wallpaper: cfg.current_wallpaper.clone(),
                pausa_manual: estado.pausa_manual.load(Ordering::Relaxed),
                hotkey: estado
                    .hotkey_activo
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
                hotkey_siguiente: estado
                    .hotkey_siguiente_activo
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
                pantalla_bloqueada: estado.pantalla_bloqueada.load(Ordering::Relaxed),
                mpv_disponible: estado.mpv_disponible,
                ffmpeg_disponible: estado.ffmpeg_disponible,
                monitores: estado.pantallas.iter().map(|p| p.nombre.clone()).collect(),
                fondos_por_monitor: cfg.fondos_por_monitor.clone(),
                uptime_seg: estado.inicio.elapsed().as_secs(),
            };
            let mut valor = serde_json::to_value(estado_daemon)
                .unwrap_or_else(|_| serde_json::json!({ "ok": false }));
            if let Some(obj) = valor.as_object_mut() {
                obj.insert("ok".into(), serde_json::json!(true));
            }
            valor
        }
        "set_wallpaper" => {
            let Some(path) = cmd.path else {
                return serde_json::json!({ "ok": false, "error": "falta path" });
            };
            let video = PathBuf::from(&path);
            if !video.is_file() {
                return serde_json::json!({
                    "ok": false,
                    "error": "el archivo no existe o no es un archivo regular"
                });
            }
            if !core::extension_ok(&video) {
                return serde_json::json!({ "ok": false, "error": "formato no soportado" });
            }
            log::info!("🎬 Cambiando a {} (monitor: {:?})", path, cmd.monitor);
            match cmd.monitor {
                Some(monitor) => match aplicar_monitor(estado, &monitor, &video) {
                    Ok(_) => serde_json::json!({ "ok": true }),
                    Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
                },
                None => {
                    aplicar_global(estado, &video);
                    serde_json::json!({ "ok": true })
                }
            }
        }
        "clear" => {
            log::info!("🧹 Quitando fondo");
            quitar_fondo(estado);
            serde_json::json!({ "ok": true })
        }
        "pause" => {
            alternar_pausa(estado, true);
            serde_json::json!({ "ok": true })
        }
        "resume" => {
            alternar_pausa(estado, false);
            serde_json::json!({ "ok": true })
        }
        "toggle_pause" => {
            let pausar = !estado.pausa_manual.load(Ordering::Relaxed);
            alternar_pausa(estado, pausar);
            serde_json::json!({ "ok": true, "pausa_manual": pausar })
        }
        "siguiente" => {
            if core::listar_fondos(&estado.dir).is_empty() {
                serde_json::json!({ "ok": false, "error": "no hay fondos" })
            } else {
                avanzar_siguiente(estado, false);
                serde_json::json!({ "ok": true })
            }
        }
        "aleatorio" => {
            if core::listar_fondos(&estado.dir).is_empty() {
                serde_json::json!({ "ok": false, "error": "no hay fondos" })
            } else {
                avanzar_aleatorio(estado, false);
                serde_json::json!({ "ok": true })
            }
        }
        "show_menu" => {
            lanzar_menu(&estado.dir);
            serde_json::json!({ "ok": true })
        }
        "save_config" => {
            if let Some(nueva) = cmd.config {
                log::info!("⚙️  Guardando configuración");
                guardar_config(estado, nueva);
            }
            serde_json::json!({ "ok": true })
        }
        "quit" => {
            log::info!("👋 Saliendo por petición");
            limpiar_salida(estado);
            std::process::exit(0);
        }
        otro => {
            log::warn!("comando desconocido: {}", otro);
            serde_json::json!({ "ok": false, "error": "comando desconocido" })
        }
    }
}

/// Aplica una configuración nueva conservando lo que gestiona el daemon
/// (fondo actual, overrides por monitor) y reaccionando a los cambios.
fn guardar_config(estado: &Arc<Estado>, nueva: Config) {
    let (fps_anterior, fps_nueva);
    let (hotkey_anterior, hotkey_siguiente_anterior);
    let (hotkey_nueva, hotkey_siguiente_nueva);
    let autostart_anterior;
    {
        let mut cfg = estado.config.lock().unwrap_or_else(|e| e.into_inner());
        fps_anterior = cfg.fps_limit;
        hotkey_anterior = cfg.hotkey.clone();
        hotkey_siguiente_anterior = cfg.hotkey_siguiente.clone();
        autostart_anterior = cfg.autostart;

        let actual = cfg.current_wallpaper.clone();

        *cfg = nueva;

        // El daemon es la fuente de verdad del fondo en uso
        cfg.current_wallpaper = actual;
        fps_nueva = cfg.fps_limit;
        hotkey_nueva = cfg.hotkey.clone();
        hotkey_siguiente_nueva = cfg.hotkey_siguiente.clone();
        if let Err(e) = cfg.guardar() {
            log::warn!("no se pudo guardar la configuración: {}", e);
        }
    }

    // Autostart: crear/eliminar .desktop
    if let Ok(exe) = std::env::current_exe() {
        let autostart = estado
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .autostart;
        if autostart != autostart_anterior || autostart {
            core::aplicar_autostart(autostart, &exe);
        }
    }

    // Atajos globales si cambió alguno
    if hotkey_nueva != hotkey_anterior || hotkey_siguiente_nueva != hotkey_siguiente_anterior {
        reregistrar_hotkeys(estado, &hotkey_nueva, &hotkey_siguiente_nueva);
    }

    // Reiniciar mpv si cambió el límite de FPS
    if fps_nueva != fps_anterior {
        log::info!(
            "🎞️  FPS limit {} → {}, reiniciando mpv",
            fps_anterior,
            fps_nueva
        );
        let cfg = estado
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let pausado = estado.pausa_manual.load(Ordering::Relaxed);
        for p in &estado.pantallas {
            if fondo_de_pantalla(&cfg, &p.nombre).is_some() {
                let _ = relanzar_pantalla_mpv(&cfg, p, pausado);
            }
        }
    }

    // Aplicar posibles cambios de overrides por monitor o fondo global
    let cfg = estado
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    sincronizar_pantallas(estado, &cfg);
}

/// Detiene los reproductores mpv y elimina los sockets al salir,
/// para no dejar procesos huérfanos reproduciendo sobre el escritorio.
fn limpiar_salida(estado: &Estado) {
    for p in &estado.pantallas {
        matar_mpv(p);
    }
    core::eliminar_socket(&core::socket_ipc());
    for p in &estado.pantallas {
        core::eliminar_socket(&p.socket);
    }
}

fn lanzar_menu(dir: &Path) {
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let menu_bin = exe_dir.join("wallpaper-menu");
    match Command::new(&menu_bin)
        .env("WALLPAPERS_DIR", dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut hijo) => {
            log::info!("✅ menú lanzado");
            // Recolectar el proceso para no dejar zombis cuando el menú se cierre
            thread::spawn(move || {
                let _ = hijo.wait();
            });
        }
        Err(e) => log::error!("menú: {}", e),
    }
}

// ── Bandeja del sistema ────────────────────────────────────────

struct Bandeja {
    estado: Arc<Estado>,
}

impl ksni::Tray for Bandeja {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "wallpaper-animado".into()
    }

    fn title(&self) -> String {
        "Azura".into()
    }

    fn icon_name(&self) -> String {
        "wallpaper-animado".into()
    }

    fn tool_tip(&self) -> ToolTip {
        let cfg = self.estado.config.lock().unwrap_or_else(|e| e.into_inner());
        let descripcion = cfg
            .current_wallpaper
            .as_ref()
            .map(|p| core::nombre_base(Path::new(p)))
            .unwrap_or_else(|| "Sin fondo".into());
        ToolTip {
            title: "Azura".into(),
            description: descripcion,
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<KsniMenuItem<Self>> {
        let pausado = self.estado.pausa_manual.load(Ordering::Relaxed);
        vec![
            StandardItem {
                label: "Abrir menú".into(),
                icon_name: "wallpaper-animado".into(),
                activate: Box::new(|this: &mut Self| {
                    lanzar_menu(&this.estado.dir);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Fondo aleatorio".into(),
                icon_name: "media-playlist-shuffle".into(),
                activate: Box::new(|this: &mut Self| {
                    avanzar_aleatorio(&this.estado, true);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Siguiente fondo".into(),
                icon_name: "media-skip-forward".into(),
                activate: Box::new(|this: &mut Self| {
                    avanzar_siguiente(&this.estado, true);
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Pausar fondo".into(),
                checked: pausado,
                activate: Box::new(|this: &mut Self| {
                    let pausar = !this.estado.pausa_manual.load(Ordering::Relaxed);
                    alternar_pausa(&this.estado, pausar);
                }),
                ..Default::default()
            }
            .into(),
            KsniMenuItem::Separator,
            StandardItem {
                label: "Salir".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    limpiar_salida(&this.estado);
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
