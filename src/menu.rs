// GUI para gestionar y seleccionar fondos animados + ajustes
use gdk4_x11::X11Surface;
use gtk4::prelude::*;
use gtk4::{
    gdk, gio, glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, ContentFit,
    CssProvider, DragSource, DropDown, DropTarget, Entry, EventControllerKey,
    EventControllerMotion, FileDialog, FileFilter, FlowBox, FlowBoxChild, GestureClick, HeaderBar,
    Image, Label, ListBox, ListBoxRow, Orientation, Overlay, Picture, Revealer,
    RevealerTransitionType, ScrolledWindow, SearchEntry, SelectionMode, SpinButton, Stack,
    StackTransitionType, StringList, Switch, ToggleButton,
};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::time::Duration;
use wallpaper_core::{self as core, Config};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    ConfigureWindowAux, ConnectionExt as _, CreateWindowAux, WindowClass,
};
use x11rb::rust_connection::RustConnection;

const APP_ID: &str = "com.mint.wallpaper-animado";

const CSS: &str = r#"
/* ── Paleta propia (oscuro) ─────────────────────────────────── */
@define-color st_bg        #0d0f16;
@define-color st_bg2       #12151f;
@define-color st_surface   #171b27;
@define-color st_surface2  #1d2231;
@define-color st_surface3  #252b3c;
@define-color st_border    alpha(#ffffff, 0.08);
@define-color st_border2   alpha(#ffffff, 0.15);
@define-color st_text      #eaeef8;
@define-color st_dim       alpha(#eaeef8, 0.62);
@define-color st_dim2      alpha(#eaeef8, 0.42);
@define-color st_accent    #8b7bff;
@define-color st_accent2   #4fe3c1;
@define-color st_danger    #e6496a;

/* ── Ventana y cabecera ─────────────────────────────────────── */
window {
  background-color: @st_bg;
  color: @st_text;
}
headerbar {
  background-color: @st_bg2;
  background-image: linear-gradient(to bottom, #171b27, #12151f);
  border-bottom: 1px solid @st_border;
  box-shadow: none;
  min-height: 58px;
  padding: 6px 10px;
}
headerbar .title {
  font-weight: 800;
  font-size: 1.1em;
}
headerbar button {
  border-radius: 10px;
}

/* ── Lateral ────────────────────────────────────────────────── */
.sidebar {
  background-color: @st_bg2;
  border-right: 1px solid @st_border;
  padding: 18px 14px;
}
.brand-nombre { font-weight: 800; font-size: 1.06em; color: @st_text; }
.brand-sub { font-size: 0.7em; font-weight: 700; color: @st_dim2; }
.nav-btn {
  border-radius: 13px;
  padding: 11px 14px;
  min-height: 44px;
  background-color: transparent;
  border: 1px solid transparent;
  color: @st_dim;
  font-weight: 700;
  font-size: 0.96em;
  transition: background-color 160ms ease, color 160ms ease, border-color 160ms ease;
}
.nav-btn:hover { background-color: alpha(#ffffff, 0.05); color: @st_text; }
.nav-btn:checked {
  background-color: alpha(@st_accent, 0.16);
  border-color: alpha(@st_accent, 0.40);
  color: @st_text;
}
.info-card {
  background-color: @st_surface;
  border: 1px solid @st_border;
  border-radius: 16px;
  padding: 14px;
}
.info-label { font-size: 0.82em; font-weight: 600; color: @st_dim; }
.dot-on { color: @st_accent2; font-size: 0.85em; }
.dot-off { color: @st_danger; font-size: 0.85em; }

/* ── Botones ────────────────────────────────────────────────── */
.btn-primary {
  border-radius: 13px;
  padding: 12px 18px;
  min-height: 44px;
  font-weight: 800;
  font-size: 0.95em;
  color: #ffffff;
  background-image: linear-gradient(135deg, @st_accent, @st_accent2);
  border: 1px solid alpha(#ffffff, 0.14);
  box-shadow: 0 6px 18px alpha(@st_accent, 0.28);
  transition: box-shadow 160ms ease, background-color 160ms ease;
}
.btn-primary:hover { box-shadow: 0 8px 24px alpha(@st_accent, 0.45); }
.btn-primary image { color: #ffffff; }
.btn-ghost {
  border-radius: 12px;
  padding: 9px 14px;
  min-height: 38px;
  font-weight: 700;
  font-size: 0.9em;
  color: @st_text;
  background-color: alpha(#ffffff, 0.06);
  border: 1px solid @st_border2;
  transition: background-color 160ms ease, border-color 160ms ease;
}
.btn-ghost:hover { background-color: alpha(#ffffff, 0.12); }
.btn-hero {
  border-radius: 12px;
  padding: 10px 16px;
  min-height: 40px;
  font-weight: 700;
  font-size: 0.9em;
  color: #ffffff;
  background-color: rgba(255,255,255,0.12);
  border: 1px solid rgba(255,255,255,0.20);
  transition: background-color 160ms ease;
}
.btn-hero:hover { background-color: rgba(255,255,255,0.22); }
.btn-hero image { color: #ffffff; }
.btn-danger {
  border-radius: 12px;
  padding: 9px 18px;
  min-height: 38px;
  font-weight: 700;
  font-size: 0.9em;
  color: #ffffff;
  background-color: @st_danger;
  border: 1px solid alpha(#ffffff, 0.16);
  transition: background-color 160ms ease;
}
.btn-danger:hover { background-color: #ff5f7d; }
.btn-danger image { color: #ffffff; }

/* ── Destacado (fondo en uso) ───────────────────────────────── */
.hero {
  border-radius: 22px;
  background-color: @st_surface;
  border: 1px solid @st_border;
  box-shadow: 0 12px 30px rgba(0,0,0,0.35);
}
.hero-pic {
  border-radius: 22px;
  background-color: @st_surface3;
}
.hero-scrim {
  border-radius: 22px;
  background-image: linear-gradient(to right, rgba(8,10,16,0.95), rgba(8,10,16,0.72) 45%, rgba(8,10,16,0.12));
}
.hero-titulo { font-size: 1.35em; font-weight: 800; color: #ffffff; }
.hero-sub { font-size: 0.82em; color: alpha(#ffffff, 0.62); }

/* ── Píldoras / etiquetas ───────────────────────────────────── */
.chip {
  background-color: alpha(#ffffff, 0.08);
  border: 1px solid alpha(#ffffff, 0.10);
  border-radius: 999px;
  padding: 3px 10px;
  font-size: 0.72em;
  font-weight: 800;
  color: @st_dim;
}
.chip-vivo {
  background-color: alpha(@st_accent, 0.22);
  border-color: alpha(@st_accent, 0.45);
  color: #d7d0ff;
}
button.chip-tag {
  padding: 2px 9px;
  min-height: 0;
  font-size: 0.7em;
  font-weight: 700;
  color: #b9f7e6;
  background-color: alpha(@st_accent2, 0.12);
  border: 1px solid alpha(@st_accent2, 0.35);
  border-radius: 999px;
}
button.chip-tag:hover {
  background-color: alpha(@st_accent2, 0.25);
  color: #ffffff;
}

/* ── Barra de herramientas ──────────────────────────────────── */
.search {
  background-color: @st_surface2;
  border: 1px solid @st_border;
  border-radius: 12px;
  padding: 6px 12px;
  min-height: 38px;
  color: @st_text;
}
.search:focus { border-color: alpha(@st_accent, 0.65); background-color: @st_surface3; }
.chips button {
  border-radius: 999px;
  padding: 7px 14px;
  min-height: 34px;
  font-weight: 700;
  font-size: 0.85em;
  color: @st_dim;
  background-color: alpha(#ffffff, 0.05);
  border: 1px solid transparent;
  transition: background-color 160ms ease, color 160ms ease, border-color 160ms ease;
}
.chips button:hover { color: @st_text; background-color: alpha(#ffffff, 0.10); }
.chips button:checked {
  background-color: alpha(@st_accent, 0.20);
  border-color: alpha(@st_accent, 0.40);
  color: #ffffff;
}
.contador { font-size: 0.85em; font-weight: 700; color: @st_dim2; }

/* ── Rejilla de fondos ──────────────────────────────────────── */
flowboxchild {
  background-color: transparent;
  border-radius: 22px;
  padding: 4px;
}
.wp-card {
  background-color: @st_surface;
  border: 1px solid @st_border;
  border-radius: 18px;
  padding: 8px 8px 8px 8px;
  transition: background-color 180ms ease, border-color 180ms ease, box-shadow 180ms ease;
}
.wp-card.hovered {
  background-color: @st_surface2;
  border-color: alpha(@st_accent, 0.35);
  box-shadow: 0 10px 26px rgba(0,0,0,0.45);
}
.wp-card.dragging { opacity: 0.45; }
.wp-card.active {
  background-color: alpha(@st_accent, 0.10);
  border-color: alpha(@st_accent, 0.55);
}
.thumb {
  border-radius: 12px;
  background-color: @st_surface3;
  min-height: 134px;
}
.wp-hover {
  background-color: alpha(#0a0c14, 0.55);
  border-radius: 12px;
  opacity: 0;
  transition: opacity 180ms ease;
}
.wp-card.hovered .wp-hover { opacity: 1; }
.hover-play {
  border-radius: 999px;
  padding: 0;
  min-width: 48px;
  min-height: 48px;
  color: #ffffff;
  background-image: linear-gradient(135deg, @st_accent, @st_accent2);
  border: 1px solid alpha(#ffffff, 0.22);
  box-shadow: 0 6px 18px rgba(0,0,0,0.45);
  transition: box-shadow 160ms ease;
}
.hover-play:hover { box-shadow: 0 8px 22px rgba(0,0,0,0.6); }
.hover-play image { color: #ffffff; }
.hover-del {
  border-radius: 999px;
  padding: 0;
  min-width: 32px;
  min-height: 32px;
  color: #ffd7de;
  background-color: rgba(10,12,20,0.72);
  border: 1px solid alpha(#ffffff, 0.16);
  transition: background-color 160ms ease, color 160ms ease;
}
.hover-del:hover { background-color: @st_danger; color: #ffffff; }
.hover-del image { color: inherit; }
.hover-prev {
  border-radius: 999px;
  padding: 0;
  min-width: 32px;
  min-height: 32px;
  color: #eaeef8;
  background-color: rgba(10,12,20,0.72);
  border: 1px solid alpha(#ffffff, 0.16);
  transition: background-color 160ms ease;
}
.hover-prev:hover { background-color: alpha(@st_accent, 0.75); color: #ffffff; }
.hover-prev image { color: inherit; }
.fav-btn {
  border-radius: 999px;
  padding: 0;
  min-width: 30px;
  min-height: 30px;
  background-color: transparent;
  border: 1px solid transparent;
  color: @st_dim2;
  transition: color 160ms ease, background-color 160ms ease;
}
.fav-btn:hover { background-color: alpha(#ffffff, 0.08); color: #ffd166; }
.fav-btn.fav-on { color: #ffd166; }
.preview-shell { background-color: #05070c; }
.preview-bar {
  background-color: #0a0c12;
  border-top: 1px solid rgba(255,255,255,0.08);
}
.deps-warn { font-size: 0.82em; color: #ffb4a6; }
.badge-activo {
  background-color: rgba(8,10,16,0.78);
  border: 1px solid alpha(@st_accent2, 0.55);
  border-radius: 999px;
  padding: 4px 10px;
}
.badge-text { font-size: 0.72em; font-weight: 800; color: #b9f7e6; }
.wp-nombre { font-weight: 700; font-size: 0.95em; color: @st_text; }

/* ── Estado vacío ───────────────────────────────────────────── */
.empty-icon { color: @st_dim2; }
.empty-titulo { font-size: 1.45em; font-weight: 800; color: @st_text; }
.empty-sub { font-size: 0.95em; color: @st_dim; }

/* ── Ajustes ────────────────────────────────────────────────── */
.seccion-titulo { font-size: 0.78em; font-weight: 800; color: @st_dim2; }
.set-tile {
  background-color: @st_surface2;
  border: 1px solid @st_border;
  border-radius: 11px;
  min-width: 38px;
  min-height: 38px;
}
.set-tile image { color: @st_accent2; }
.settings-card {
  background-color: @st_surface;
  border: 1px solid @st_border;
  border-radius: 18px;
}
listbox.settings-card {
  background-color: transparent;
  border: none;
  box-shadow: none;
}
row.settings-row {
  background-color: transparent;
  padding: 0;
}
row.settings-row:hover { background-color: alpha(#ffffff, 0.03); }
settings-row label.titulo { font-weight: 700; font-size: 0.96em; color: @st_text; }
settings-row label.sub { font-size: 0.82em; color: @st_dim2; }
separator {
  background-color: @st_border;
  min-height: 1px;
}
switch {
  background-color: alpha(#ffffff, 0.10);
  border: 1px solid @st_border2;
  border-radius: 999px;
  min-width: 48px;
  min-height: 26px;
  box-shadow: none;
  transition: background-color 160ms ease;
}
switch:checked {
  background-color: alpha(@st_accent, 0.30);
  background-image: linear-gradient(135deg, @st_accent, @st_accent2);
  border-color: transparent;
}
switch slider {
  background-color: #ffffff;
  border: none;
  border-radius: 999px;
  min-width: 20px;
  min-height: 20px;
  margin: 2px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.45);
}
switch image { color: #ffffff; }
switch:not(:checked) image { color: alpha(@st_text, 0.0); }
spinbutton {
  background-color: @st_surface2;
  border: 1px solid @st_border;
  border-radius: 11px;
  padding: 2px;
  color: @st_text;
  min-height: 34px;
}
spinbutton text { color: @st_text; font-weight: 700; }
spinbutton button {
  background-color: transparent;
  border: none;
  color: @st_dim;
  min-width: 28px;
}
spinbutton button:hover { background-color: alpha(#ffffff, 0.08); color: @st_text; }
.notas { font-size: 0.85em; color: @st_dim2; }

/* ── Aviso emergente y diálogo ──────────────────────────────── */
.toast {
  background-color: @st_surface3;
  border: 1px solid @st_border2;
  border-radius: 14px;
  padding: 11px 16px;
  box-shadow: 0 12px 30px rgba(0,0,0,0.55);
  color: @st_text;
  font-weight: 700;
}
.toast image { color: @st_accent2; }
.lightbox { background-color: rgba(5,7,12,0.62); }
.dialog-card {
  background-color: @st_surface2;
  border: 1px solid @st_border2;
  border-radius: 20px;
  padding: 22px;
  box-shadow: 0 20px 50px rgba(0,0,0,0.6);
}
.dialog-titulo { font-size: 1.15em; font-weight: 800; color: @st_text; }
.dialog-sub { font-size: 0.9em; color: @st_dim; }

/* ── Arrastrar y soltar ─────────────────────────────────────── */
.drop-zone {
  border: 2px dashed alpha(@st_accent, 0.55);
  border-radius: 22px;
  background-color: alpha(@st_accent, 0.08);
}

/* ── Barras de desplazamiento ───────────────────────────────── */
scrollbar { background-color: transparent; }
scrollbar trough { background-color: transparent; min-width: 12px; }
scrollbar slider {
  background-color: alpha(#ffffff, 0.16);
  border-radius: 999px;
  min-width: 8px;
  min-height: 8px;
  margin: 4px;
  transition: background-color 160ms ease;
}
scrollbar slider:hover { background-color: alpha(#ffffff, 0.32); }
"#;

#[derive(Clone, Copy, PartialEq)]
enum Filtro {
    Todo,
    Video,
    Imagen,
    Favoritos,
}

#[derive(Clone, Copy, PartialEq)]
enum Orden {
    NombreAz,
    NombreZa,
    Fecha,
    Tamano,
    Manual,
}

impl Orden {
    fn id(self) -> &'static str {
        match self {
            Orden::NombreAz => "nombre-az",
            Orden::NombreZa => "nombre-za",
            Orden::Fecha => "fecha",
            Orden::Tamano => "tamano",
            Orden::Manual => "manual",
        }
    }

    fn desde_id(id: &str) -> Self {
        match id {
            "nombre-za" => Orden::NombreZa,
            "fecha" => Orden::Fecha,
            "tamano" => Orden::Tamano,
            "manual" => Orden::Manual,
            _ => Orden::NombreAz,
        }
    }
}

struct ItemGrid {
    child: FlowBoxChild,
    path: PathBuf,
    nombre: String,
    es_video: bool,
}

struct AppUi {
    config: RefCell<Config>,
    dir: PathBuf,
    flow: FlowBox,
    paginas_biblio: Stack,
    items: RefCell<Vec<ItemGrid>>,
    contador: Label,
    hero_pic: Picture,
    hero_titulo: Label,
    hero_sub: Label,
    hero_chips: GtkBox,
    btn_pausa: Button,
    btn_quitar: Button,
    hero_monitor: GtkBox,
    monitor_dd: DropDown,
    monitor_sel: RefCell<Vec<String>>,
    toast: Revealer,
    toast_icon: Image,
    toast_label: Label,
    toast_gen: Cell<u64>,
    filtro: Cell<Filtro>,
    query: RefCell<String>,
    search: SearchEntry,
    chip_todo: ToggleButton,
    dot_daemon: Label,
    txt_daemon: Label,
    lbl_hotkey: Label,
    lbl_deps: Label,
    lbl_info_hotkey: Label,
    lbl_info_hotkey_siguiente: Label,
    lbl_info_servidor: Label,
    btn_iniciar: Button,
    cuenta_lateral: Label,
    lightbox: Overlay,
    lightbox_titulo: Label,
    lightbox_sub: Label,
    pendiente: RefCell<Option<PathBuf>>,
    lista_perfiles: ListBox,
}

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| cargar_css());
    app.connect_activate(build_ui);
    app.run();
}

fn cargar_css() {
    // Forzar tema oscuro: toda la paleta de la app está pensada sobre fondo oscuro.
    if let Some(settings) = gtk4::Settings::default() {
        settings.set_property("gtk-application-prefer-dark-theme", true);
    }

    let provider = CssProvider::new();
    provider.load_from_bytes(&glib::Bytes::from_static(CSS.as_bytes()));
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

// ── Utilidades ─────────────────────────────────────────────────

/// Genera (y cachea) una miniatura del fondo. `ancho` es el ancho en píxeles
/// y `sufijo` distingue la variante de la caché (tarjeta pequeña / destacado).
fn generar_thumbnail(path: &Path, ancho: u32, sufijo: &str) -> Option<PathBuf> {
    let clave = core::clave_cache(path);
    let out = core::cache_dir().join(format!("{}{}.jpg", clave, sufijo));
    if out.exists() {
        return Some(out);
    }
    let escala = format!("scale={}:-2", ancho);
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            path.to_str()?,
            "-vf",
            &escala,
            "-frames:v",
            "1",
            "-q:v",
            "4",
            out.to_str()?,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    if status.success() {
        Some(out)
    } else {
        None
    }
}

/// Miniatura para las tarjetas de la biblioteca (pequeña).
fn thumb_tarjeta(path: &Path) -> Option<PathBuf> {
    generar_thumbnail(path, 280, "_sm")
}

/// Miniatura grande para el destacado.
fn thumb_destacado(path: &Path) -> Option<PathBuf> {
    generar_thumbnail(path, 960, "")
}

fn borrar_miniaturas(path: &Path) {
    let clave = core::clave_cache(path);
    let cache = core::cache_dir();
    let _ = std::fs::remove_file(cache.join(format!("{}.jpg", clave)));
    let _ = std::fs::remove_file(cache.join(format!("{}_sm.jpg", clave)));
    let _ = std::fs::remove_file(cache.join(format!("{}.meta.json", clave)));
}

/// Metadatos del fondo (cacheados en disco por contenido).
fn meta_de(path: &Path) -> Option<core::MetaFondo> {
    let cache = core::cache_dir().join(format!("{}.meta.json", core::clave_cache(path)));
    if let Ok(texto) = std::fs::read_to_string(&cache) {
        if let Ok(meta) = serde_json::from_str::<core::MetaFondo>(&texto) {
            return Some(meta);
        }
    }
    let meta = core::leer_metadatos(path)?;
    if let Ok(texto) = serde_json::to_string(&meta) {
        let _ = std::fs::write(&cache, texto);
    }
    Some(meta)
}

/// Etiquetas guardadas para un fondo.
fn etiquetas_de(cfg: &Config, path: &Path) -> Vec<String> {
    cfg.etiquetas
        .get(path.to_string_lossy().as_ref())
        .cloned()
        .unwrap_or_default()
}

/// Resumen de una importación (para avisos de duplicados).
struct ResultadoImport {
    añadidos: usize,
    duplicados: usize,
}

fn extension_de(path: &Path) -> String {
    path.extension()
        .map(|x| x.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| "FILE".into())
}

fn chip(texto: &str) -> Label {
    let l = Label::new(Some(texto));
    l.add_css_class("chip");
    l
}

fn limpiar(caja: &GtkBox) {
    while let Some(h) = caja.first_child() {
        caja.remove(&h);
    }
}

fn icono(nombre: &str, px: i32) -> Image {
    let i = Image::from_icon_name(nombre);
    i.set_pixel_size(px);
    i
}

fn boton_con_icono(clase: &str, icono_nombre: &str, texto: &str) -> Button {
    let b = Button::new();
    b.add_css_class(clase);
    let caja = GtkBox::new(Orientation::Horizontal, 8);
    caja.append(&icono(icono_nombre, 16));
    caja.append(&Label::new(Some(texto)));
    b.set_child(Some(&caja));
    b
}

fn aleatorio(n: usize) -> usize {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as usize;
    nanos % n.max(1)
}

/// Fecha de modificación en segundos (0 si no se puede leer).
fn mtime_de(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .ok()
        })
        .unwrap_or(0)
}

/// Tamaño en bytes (0 si no se puede leer).
fn bytes_de(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Genera «nombre (n).ext» hasta encontrar un nombre libre en `dir`.
fn nombre_unico(dir: &Path, src: &Path) -> Option<PathBuf> {
    let stem = src.file_stem()?.to_string_lossy().to_string();
    let ext = src.extension().map(|e| e.to_string_lossy().to_string());
    for i in 1..1000 {
        let candidato = match &ext {
            Some(e) => format!("{} ({}).{}", stem, i, e),
            None => format!("{} ({})", stem, i),
        };
        let ruta = dir.join(candidato);
        if !ruta.exists() {
            return Some(ruta);
        }
    }
    None
}

/// Actualiza el icono y el estilo del botón de favorito.
fn mostrar_favorito(boton: &Button, activo: bool) {
    boton.set_icon_name(if activo {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    });
    if activo {
        boton.add_css_class("fav-on");
    } else {
        boton.remove_css_class("fav-on");
    }
}

/// Sincroniza icono y texto del botón pausar/reanudar.
fn actualizar_boton_pausa(boton: &Button, pausado: bool) {
    let icono = if pausado {
        "media-playback-start-symbolic"
    } else {
        "media-playback-pause-symbolic"
    };
    let texto = if pausado { "Reanudar" } else { "Pausar" };
    if let Some(caja) = boton.child().and_downcast::<GtkBox>() {
        if let Some(img) = caja.first_child().and_downcast::<Image>() {
            img.set_icon_name(Some(icono));
        }
        if let Some(lbl) = caja.last_child().and_downcast::<Label>() {
            lbl.set_text(texto);
        }
    }
}

fn daemon_conectado() -> bool {
    core::consultar_estado().is_some()
}

fn enviar_comando(cmd: &serde_json::Value) -> bool {
    core::enviar_ipc(cmd).is_ok()
}

fn enviar_guardar_config(config: &RefCell<Config>) {
    let json = serde_json::json!({
        "cmd": "save_config",
        "config": *config.borrow(),
    });
    let _ = core::enviar_ipc(&json);
}

/// Ruta de un binario instalado junto al ejecutable actual.
fn binario_hermano(nombre: &str) -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(nombre)))
}

/// Arranca el daemon si está apagado y refresca la interfaz al conectar.
fn iniciar_daemon(ui: &Rc<AppUi>) {
    if daemon_conectado() {
        ui.toast(
            "El servidor ya está en ejecución",
            "dialog-information-symbolic",
        );
        ui.refrescar_daemon();
        return;
    }
    let Some(bin) = binario_hermano("wallpaper-daemon") else {
        ui.toast(
            "No se encontró el binario del servidor",
            "dialog-error-symbolic",
        );
        return;
    };
    if Command::new(&bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_err()
    {
        ui.toast("No se pudo iniciar el servidor", "dialog-error-symbolic");
        return;
    }
    // Reintentar la conexión durante unos segundos sin bloquear la interfaz
    let ui2 = ui.clone();
    let mut intentos = 0u32;
    glib::timeout_add_local(Duration::from_millis(400), move || {
        intentos += 1;
        if daemon_conectado() {
            ui2.toast("Servidor iniciado", "emblem-ok-symbolic");
            ui2.refrescar_daemon();
            glib::ControlFlow::Break
        } else if intentos >= 15 {
            ui2.toast("El servidor no respondió", "dialog-warning-symbolic");
            ui2.refrescar_daemon();
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn abrir_carpeta(dir: &Path) {
    let uri = format!("file://{}", dir.to_string_lossy());
    let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
}

// ── Lógica de la interfaz ──────────────────────────────────────

impl AppUi {
    fn recargar(self: &Rc<Self>) {
        while let Some(h) = self.flow.first_child() {
            self.flow.remove(&h);
        }
        self.items.borrow_mut().clear();

        let actual = self.config.borrow().current_wallpaper.clone();
        let orden = Orden::desde_id(&self.config.borrow().orden_biblioteca);
        let mut fondos = core::listar_fondos(&self.dir);
        match orden {
            Orden::NombreAz => {}
            Orden::NombreZa => fondos.reverse(),
            Orden::Fecha => {
                fondos.sort_by_key(|p| std::cmp::Reverse(mtime_de(p)));
            }
            Orden::Tamano => {
                fondos.sort_by_key(|p| std::cmp::Reverse(bytes_de(p)));
            }
            Orden::Manual => {
                let cfg = self.config.borrow();
                let posicion = |p: &PathBuf| {
                    cfg.orden_manual
                        .iter()
                        .position(|r| *r == p.to_string_lossy())
                        .unwrap_or(usize::MAX)
                };
                fondos.sort_by_key(|p| (posicion(p), p.file_name().map(|n| n.to_owned())));
            }
        }
        for path in fondos {
            let es_actual = actual
                .as_ref()
                .map(|a| path.to_string_lossy() == a.as_str())
                .unwrap_or(false);
            let item = crear_tarjeta(self, &path, es_actual);
            self.flow.insert(&item.child, -1);
            self.items.borrow_mut().push(item);
        }

        let total = self.items.borrow().len();
        self.cuenta_lateral
            .set_text(&format!("{} fondos en la biblioteca", total));
        self.aplicar_visibilidad();
        self.actualizar_hero();
        self.refrescar_daemon();
    }

    fn aplicar_visibilidad(&self) {
        let q = self.query.borrow().to_lowercase();
        let filtro = self.filtro.get();
        let favoritos: Vec<String> = self.config.borrow().favoritos.clone();
        let etiquetas = self.config.borrow().etiquetas.clone();
        let mut visibles = 0usize;
        let total;
        {
            let items = self.items.borrow();
            total = items.len();
            for item in items.iter() {
                let por_tipo = match filtro {
                    Filtro::Todo => true,
                    Filtro::Video => item.es_video,
                    Filtro::Imagen => !item.es_video,
                    Filtro::Favoritos => {
                        let ruta = item.path.to_string_lossy();
                        favoritos.iter().any(|f| f == ruta.as_ref())
                    }
                };
                let por_texto = q.is_empty()
                    || item.nombre.to_lowercase().contains(&q)
                    || etiquetas
                        .get(item.path.to_string_lossy().as_ref())
                        .map(|tags| tags.iter().any(|t| t.to_lowercase().contains(&q)))
                        .unwrap_or(false);
                let visible = por_tipo && por_texto;
                item.child.set_visible(visible);
                if visible {
                    visibles += 1;
                }
            }
        }
        self.contador.set_text(&if visibles == total {
            format!("{} fondos", total)
        } else {
            format!("{} de {} fondos", visibles, total)
        });
        // Página de la biblioteca: vacía, sin coincidencias o rejilla
        self.paginas_biblio.set_visible_child_name(if total == 0 {
            "vacio"
        } else if visibles == 0 {
            "sin"
        } else {
            "grid"
        });
    }

    /// Restablece la búsqueda y el filtro al estado por defecto.
    fn restablecer_busqueda(self: &Rc<Self>) {
        self.search.set_text("");
        self.chip_todo.set_active(true);
        *self.query.borrow_mut() = String::new();
        self.filtro.set(Filtro::Todo);
        self.aplicar_visibilidad();
    }

    fn actualizar_hero(&self) {
        let activo = self
            .config
            .borrow()
            .current_wallpaper
            .clone()
            .map(PathBuf::from)
            .filter(|p| p.exists());

        match activo {
            Some(path) => {
                let thumb = thumb_destacado(&path).unwrap_or_else(|| path.clone());
                self.hero_pic.set_filename(Some(&thumb));
                self.hero_titulo.set_text(&core::nombre_base(&path));
                self.hero_sub.set_text(path.to_string_lossy().as_ref());
                limpiar(&self.hero_chips);
                self.hero_chips.append(&chip(&extension_de(&path)));
                self.hero_chips.append(&chip(&core::peso_de(&path)));
                if core::es_video(&path) {
                    self.hero_chips.append(&chip("VÍDEO"));
                } else {
                    self.hero_chips.append(&chip("IMAGEN"));
                }
                let fijados = self.config.borrow().fondos_por_monitor.len();
                if fijados > 0 {
                    self.hero_chips
                        .append(&chip(&format!("{} MONITOR(ES) FIJOS", fijados)));
                }
            }
            None => {
                self.hero_pic.set_filename(None::<&Path>);
                self.hero_titulo.set_text("Sin fondo activo");
                self.hero_sub
                    .set_text("Elige uno de tu biblioteca o añade archivos nuevos");
                limpiar(&self.hero_chips);
                self.hero_chips.append(&chip("EN ESPERA"));
            }
        }
    }

    fn refrescar_daemon(self: &Rc<Self>) {
        let estado = core::consultar_estado();
        let activo = estado.is_some();

        self.dot_daemon.set_text(if activo { "●" } else { "○" });
        self.dot_daemon.remove_css_class("dot-on");
        self.dot_daemon.remove_css_class("dot-off");
        self.dot_daemon
            .add_css_class(if activo { "dot-on" } else { "dot-off" });
        self.txt_daemon.set_text(if activo {
            "Servidor activo"
        } else {
            "Servidor apagado"
        });
        self.btn_iniciar.set_visible(!activo);

        let Some(est) = estado else {
            self.lbl_hotkey.set_text("Atajo: —");
            self.lbl_info_hotkey.set_text("—");
            self.lbl_info_hotkey_siguiente.set_text("—");
            self.lbl_deps.set_visible(false);
            self.lbl_info_servidor.set_text("Apagado");
            self.btn_pausa.set_sensitive(false);
            self.btn_quitar.set_sensitive(false);
            return;
        };

        // Sincronizar lo que el daemon gestiona (slideshow, overrides…)
        let fondo_cambio = {
            let cfg = self.config.borrow();
            est.wallpaper != cfg.current_wallpaper
                || est.fondos_por_monitor != cfg.fondos_por_monitor
        };
        if fondo_cambio {
            let mut cfg = self.config.borrow_mut();
            cfg.current_wallpaper = est.wallpaper.clone();
            cfg.fondos_por_monitor = est.fondos_por_monitor.clone();
        }

        self.lbl_hotkey.set_text(&format!("Atajo: {}", est.hotkey));
        self.lbl_info_hotkey.set_text(&est.hotkey);
        self.lbl_info_hotkey_siguiente
            .set_text(&est.hotkey_siguiente);
        self.lbl_info_servidor
            .set_text(&format!("En ejecución ({} s)", est.uptime_seg));

        let mut avisos = Vec::new();
        if !est.mpv_disponible {
            avisos.push("mpv no instalado");
        }
        if !est.ffmpeg_disponible {
            avisos.push("ffmpeg no instalado");
        }
        if avisos.is_empty() {
            self.lbl_deps.set_visible(false);
        } else {
            self.lbl_deps.set_visible(true);
            self.lbl_deps.set_text(&format!("⚠ {}", avisos.join(" · ")));
        }

        let tiene_fondo = est.wallpaper.is_some();
        self.btn_pausa.set_sensitive(tiene_fondo);
        self.btn_quitar.set_sensitive(tiene_fondo);
        actualizar_boton_pausa(&self.btn_pausa, est.pausa_manual);

        self.refrescar_monitores(&est.monitores);

        if fondo_cambio {
            // El fondo cambió en el daemon (p. ej. slideshow): reconstruir la vista
            self.recargar();
        }
    }

    /// Reconstruye el selector de monitor solo si la lista cambió.
    fn refrescar_monitores(self: &Rc<Self>, monitores: &[String]) {
        let mut lista = vec!["Todos".to_string()];
        lista.extend(monitores.iter().cloned());
        if *self.monitor_sel.borrow() == lista {
            return;
        }
        *self.monitor_sel.borrow_mut() = lista.clone();
        let refs: Vec<&str> = lista.iter().map(|s| s.as_str()).collect();
        let modelo = StringList::new(&refs);
        self.monitor_dd.set_model(Some(&modelo));
        self.monitor_dd.set_selected(0);
        self.hero_monitor.set_visible(monitores.len() > 1);
    }

    /// Monitor elegido en el selector; None = todos.
    fn monitor_objetivo(&self) -> Option<String> {
        let idx = self.monitor_dd.selected() as usize;
        if idx == 0 {
            return None;
        }
        self.monitor_sel.borrow().get(idx).cloned()
    }

    fn toast(self: &Rc<Self>, texto: &str, icono_nombre: &str) {
        self.toast_label.set_text(texto);
        self.toast_icon.set_icon_name(Some(icono_nombre));
        self.toast.set_reveal_child(true);
        self.toast_gen.set(self.toast_gen.get() + 1);
        let gen = self.toast_gen.get();
        let ui = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(2800), move || {
            if ui.toast_gen.get() == gen {
                ui.toast.set_reveal_child(false);
            }
        });
    }

    fn aplicar_fondo(self: &Rc<Self>, path: &Path) {
        let monitor = self.monitor_objetivo();
        let mut json = serde_json::json!({
            "cmd": "set_wallpaper",
            "path": path.to_string_lossy(),
        });
        if let Some(m) = &monitor {
            json["monitor"] = serde_json::json!(m);
        }
        {
            let mut cfg = self.config.borrow_mut();
            match &monitor {
                Some(m) => {
                    cfg.fondos_por_monitor
                        .insert(m.clone(), path.to_string_lossy().into_owned());
                }
                None => {
                    cfg.current_wallpaper = Some(path.to_string_lossy().into_owned());
                    cfg.fondos_por_monitor.clear();
                }
            }
            cfg.guardar();
        }
        let conectado = enviar_comando(&json);
        self.recargar();
        if conectado {
            let destino = monitor.map(|m| format!(" en «{}»", m)).unwrap_or_default();
            self.toast(
                &format!("Aplicando «{}»{}", core::nombre_base(path), destino),
                "media-playback-start-symbolic",
            );
        } else {
            self.toast(
                "El servidor no está en ejecución",
                "dialog-warning-symbolic",
            );
        }
    }

    fn fondo_aleatorio(self: &Rc<Self>) {
        let fondos = core::listar_fondos(&self.dir);
        if fondos.is_empty() {
            self.toast(
                "No hay fondos en la biblioteca",
                "dialog-information-symbolic",
            );
            return;
        }
        let actual = self.config.borrow().current_wallpaper.clone();
        let candidatos: Vec<&PathBuf> = fondos
            .iter()
            .filter(|p| {
                actual
                    .as_ref()
                    .map(|a| p.to_string_lossy() != a.as_str())
                    .unwrap_or(true)
            })
            .collect();
        let elegido = if candidatos.is_empty() {
            &fondos[0]
        } else {
            candidatos[aleatorio(candidatos.len())]
        };
        self.aplicar_fondo(elegido);
    }

    /// Marca o desmarca un fondo como favorito y guarda la config.
    fn alternar_favorito(self: &Rc<Self>, path: &Path, boton: &Button) {
        let ruta = path.to_string_lossy().to_string();
        let ahora_fav;
        {
            let mut cfg = self.config.borrow_mut();
            if let Some(pos) = cfg.favoritos.iter().position(|f| f == &ruta) {
                cfg.favoritos.remove(pos);
                ahora_fav = false;
            } else {
                cfg.favoritos.push(ruta);
                ahora_fav = true;
            }
            cfg.guardar();
        }
        enviar_guardar_config(&self.config);
        mostrar_favorito(boton, ahora_fav);
        self.aplicar_visibilidad();
    }

    /// Pausa o reanuda la reproducción del fondo.
    fn alternar_pausa_manual(self: &Rc<Self>) {
        let pausar = !self.config_pausa_manual();
        let cmd = if pausar { "pause" } else { "resume" };
        if !enviar_comando(&serde_json::json!({ "cmd": cmd })) {
            self.toast(
                "El servidor no está en ejecución",
                "dialog-warning-symbolic",
            );
            return;
        }
        self.refrescar_daemon();
        self.toast(
            if pausar {
                "Fondo en pausa"
            } else {
                "Fondo reanudado"
            },
            if pausar {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            },
        );
    }

    fn config_pausa_manual(&self) -> bool {
        core::consultar_estado()
            .map(|e| e.pausa_manual)
            .unwrap_or(false)
    }

    /// Quita el fondo de todas las pantallas.
    fn quitar_fondo(self: &Rc<Self>) {
        if !enviar_comando(&serde_json::json!({ "cmd": "clear" })) {
            self.toast(
                "El servidor no está en ejecución",
                "dialog-warning-symbolic",
            );
            return;
        }
        {
            let mut cfg = self.config.borrow_mut();
            cfg.current_wallpaper = None;
            cfg.fondos_por_monitor.clear();
            cfg.guardar();
        }
        self.recargar();
        self.toast("Fondo quitado", "user-trash-symbolic");
    }

    fn pedir_eliminar(self: &Rc<Self>, path: &Path) {
        *self.pendiente.borrow_mut() = Some(path.to_path_buf());
        self.lightbox_titulo.set_text("¿Eliminar este fondo?");
        self.lightbox_sub.set_text(path.to_string_lossy().as_ref());
        self.lightbox.set_visible(true);
    }

    fn cerrar_lightbox(&self) {
        self.lightbox.set_visible(false);
        *self.pendiente.borrow_mut() = None;
    }

    fn eliminar_pendiente(self: &Rc<Self>) {
        let Some(path) = self.pendiente.borrow_mut().take() else {
            self.cerrar_lightbox();
            return;
        };
        let ruta = path.to_string_lossy().to_string();
        let era_activo = self
            .config
            .borrow()
            .current_wallpaper
            .as_deref()
            .map(|a| a == ruta.as_str())
            .unwrap_or(false);

        let _ = std::fs::remove_file(&path);
        borrar_miniaturas(&path);
        {
            let mut cfg = self.config.borrow_mut();
            cfg.favoritos.retain(|f| f != &ruta);
            cfg.fondos_por_monitor.retain(|_, v| v != &ruta);
            if era_activo {
                cfg.current_wallpaper = None;
            }
            cfg.guardar();
        }
        self.cerrar_lightbox();
        self.recargar();
        enviar_guardar_config(&self.config);

        if era_activo {
            if let Some(siguiente) = core::listar_fondos(&self.dir).into_iter().next() {
                self.aplicar_fondo(&siguiente);
                return;
            }
            let _ = enviar_comando(&serde_json::json!({ "cmd": "clear" }));
        }
        self.toast("Fondo eliminado", "user-trash-symbolic");
    }

    /// Mueve `origen` justo antes/después de `destino` en el orden manual.
    fn reordenar_manual(self: &Rc<Self>, origen: &str, destino: &str, antes: bool) {
        let mut cfg = self.config.borrow_mut();
        if cfg.orden_manual.is_empty() {
            cfg.orden_manual = self
                .items
                .borrow()
                .iter()
                .map(|i| i.path.to_string_lossy().to_string())
                .collect();
        }
        let mut orden = cfg.orden_manual.clone();
        orden.retain(|r| r != origen);
        match orden.iter().position(|r| r == destino) {
            Some(pos) => orden.insert(if antes { pos } else { pos + 1 }, origen.to_string()),
            None => orden.push(origen.to_string()),
        }
        cfg.orden_manual = orden;
        cfg.orden_biblioteca = "manual".to_string();
        cfg.guardar();
        drop(cfg);
        self.recargar();
    }

    fn importar_dialogo(self: &Rc<Self>, parent: &ApplicationWindow) {
        let dialog = FileDialog::new();
        dialog.set_title("Añadir fondos animados");

        let filtros = gio::ListStore::new::<FileFilter>();
        let filtro = FileFilter::new();
        filtro.set_name(Some("Vídeos e imágenes"));
        for m in [
            "video/mp4",
            "video/webm",
            "video/x-matroska",
            "video/quicktime",
            "image/gif",
            "image/png",
            "image/jpeg",
        ] {
            filtro.add_mime_type(m);
        }
        filtros.append(&filtro);
        dialog.set_filters(Some(&filtros));

        let ui = self.clone();
        dialog.open_multiple(Some(parent), None::<&gio::Cancellable>, move |res| {
            if let Ok(lista) = res {
                let archivos: Vec<gio::File> = (0..lista.n_items())
                    .filter_map(|i| lista.item(i).and_downcast::<gio::File>())
                    .collect();
                let r = ui.importar_archivos(&archivos);
                ui.avisar_importacion(&r);
            }
        });
    }

    fn importar_archivos(self: &Rc<Self>, archivos: &[gio::File]) -> ResultadoImport {
        let enlazar = self.config.borrow().enlazar_al_importar;

        // Huellas de lo que ya hay en la biblioteca (para omitir duplicados)
        let mut huellas: Vec<u64> = core::listar_fondos(&self.dir)
            .iter()
            .filter_map(|p| core::hash_rapido(p))
            .collect();

        let mut r = ResultadoImport {
            añadidos: 0,
            duplicados: 0,
        };
        for archivo in archivos {
            let Some(src) = archivo.path() else { continue };
            if !core::extension_ok(&src) {
                continue;
            }
            let huella = core::hash_rapido(&src);
            if let Some(h) = huella {
                if huellas.contains(&h) {
                    r.duplicados += 1;
                    continue;
                }
            }
            if self.importar_uno(&src, enlazar) {
                if let Some(h) = huella {
                    huellas.push(h);
                }
                r.añadidos += 1;
            }
        }
        if r.añadidos > 0 {
            self.recargar();
        }
        r
    }

    /// Toast resumen de una importación.
    fn avisar_importacion(self: &Rc<Self>, r: &ResultadoImport) {
        match (r.añadidos, r.duplicados) {
            (0, 0) => {}
            (n, 0) => self.toast(&format!("{} fondos añadidos", n), "list-add-symbolic"),
            (0, d) => self.toast(
                &format!("{} duplicados omitidos", d),
                "dialog-warning-symbolic",
            ),
            (n, d) => self.toast(
                &format!("{} añadidos, {} duplicados omitidos", n, d),
                "list-add-symbolic",
            ),
        }
    }

    /// Importa un fondo a la biblioteca. Devuelve true si se añadió.
    fn importar_uno(&self, src: &Path, enlazar: bool) -> bool {
        let Some(nombre) = src.file_name() else {
            return false;
        };
        let dst = self.dir.join(nombre);

        // Mismo archivo que ya está en la biblioteca
        if src == dst {
            return false;
        }
        if let (Ok(a), Ok(b)) = (std::fs::canonicalize(src), std::fs::canonicalize(&dst)) {
            if a == b {
                return false;
            }
        }

        // Nombre libre si ya existe otro archivo con ese nombre
        let dst = if dst.exists() {
            match nombre_unico(&self.dir, src) {
                Some(p) => p,
                None => return false,
            }
        } else {
            dst
        };

        if enlazar {
            std::os::unix::fs::symlink(src, &dst).is_ok()
        } else {
            std::fs::copy(src, &dst).is_ok()
        }
    }
}

// ── Tarjeta de la biblioteca ───────────────────────────────────

fn crear_tarjeta(ui: &Rc<AppUi>, path: &Path, es_actual: bool) -> ItemGrid {
    let stem = core::nombre_base(path);
    let es_favorito = {
        let ruta = path.to_string_lossy();
        ui.config
            .borrow()
            .favoritos
            .iter()
            .any(|f| f == ruta.as_ref())
    };

    let card = GtkBox::new(Orientation::Vertical, 0);
    card.add_css_class("wp-card");
    if es_actual {
        card.add_css_class("active");
    }
    card.set_size_request(240, -1);

    // Miniatura
    let miniatura = thumb_tarjeta(path);
    let picture = Picture::for_filename(miniatura.as_deref().unwrap_or(path));
    picture.add_css_class("thumb");
    picture.set_content_fit(ContentFit::Cover);
    picture.set_can_shrink(true);
    picture.set_size_request(-1, 134);
    picture.set_halign(Align::Fill);
    picture.set_valign(Align::Fill);

    let overlay = Overlay::new();
    overlay.set_child(Some(&picture));

    if es_actual {
        let insignia = GtkBox::new(Orientation::Horizontal, 6);
        insignia.add_css_class("badge-activo");
        insignia.set_halign(Align::Start);
        insignia.set_valign(Align::Start);
        insignia.set_margin_top(10);
        insignia.set_margin_start(10);
        insignia.append(&icono("media-playback-start-symbolic", 12));
        let texto = Label::new(Some("Activo"));
        texto.add_css_class("badge-text");
        insignia.append(&texto);
        overlay.add_overlay(&insignia);
    }

    // Acciones que aparecen al pasar el ratón
    let hover = Overlay::new();
    hover.add_css_class("wp-hover");
    hover.set_halign(Align::Fill);
    hover.set_valign(Align::Fill);

    let play = Button::from_icon_name("media-playback-start-symbolic");
    play.add_css_class("hover-play");
    play.set_halign(Align::Center);
    play.set_valign(Align::Center);
    play.set_tooltip_text(Some("Aplicar este fondo"));
    hover.set_child(Some(&play));

    let borrar = Button::from_icon_name("user-trash-symbolic");
    borrar.add_css_class("hover-del");
    borrar.set_halign(Align::End);
    borrar.set_valign(Align::Start);
    borrar.set_margin_top(10);
    borrar.set_margin_end(10);
    borrar.set_tooltip_text(Some("Eliminar de la biblioteca"));
    hover.add_overlay(&borrar);

    let previsualizar = Button::from_icon_name("zoom-in-symbolic");
    previsualizar.add_css_class("hover-prev");
    previsualizar.set_halign(Align::End);
    previsualizar.set_valign(Align::End);
    previsualizar.set_margin_bottom(10);
    previsualizar.set_margin_end(10);
    previsualizar.set_tooltip_text(Some("Vista previa"));
    hover.add_overlay(&previsualizar);

    overlay.add_overlay(&hover);
    card.append(&overlay);

    // Meta: nombre + favorito
    let meta = GtkBox::new(Orientation::Vertical, 6);
    meta.set_margin_top(10);
    meta.set_margin_start(6);
    meta.set_margin_end(6);

    let fila_nombre = GtkBox::new(Orientation::Horizontal, 6);
    let nombre = Label::new(Some(&stem));
    nombre.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    nombre.set_max_width_chars(24);
    nombre.set_halign(Align::Start);
    nombre.set_hexpand(true);
    nombre.add_css_class("wp-nombre");
    fila_nombre.append(&nombre);

    let fav = Button::from_icon_name("non-starred-symbolic");
    fav.add_css_class("fav-btn");
    fav.set_valign(Align::Center);
    fav.set_tooltip_text(Some("Marcar como favorito"));
    mostrar_favorito(&fav, es_favorito);
    fila_nombre.append(&fav);
    meta.append(&fila_nombre);

    let fila = GtkBox::new(Orientation::Horizontal, 6);
    fila.append(&chip(&extension_de(path)));
    if let Some(meta) = meta_de(path) {
        fila.append(&chip(&meta.resolucion()));
        if meta.duracion_seg > 0.0 {
            fila.append(&chip(&meta.duracion()));
        }
    }
    fila.append(&chip(&core::peso_de(path)));
    meta.append(&fila);

    // Etiquetas (clic = buscar por esa etiqueta)
    let etiquetas = etiquetas_de(&ui.config.borrow(), path);
    if !etiquetas.is_empty() {
        let fila_tags = GtkBox::new(Orientation::Horizontal, 6);
        for etiqueta in etiquetas.iter().take(3) {
            let b = Button::with_label(etiqueta);
            b.add_css_class("chip-tag");
            b.set_tooltip_text(Some("Buscar por esta etiqueta"));
            let ui_t = ui.clone();
            let texto = etiqueta.clone();
            b.connect_clicked(move |_| ui_t.search.set_text(&texto));
            fila_tags.append(&b);
        }
        meta.append(&fila_tags);
    }
    card.append(&meta);

    let child = FlowBoxChild::new();
    child.set_child(Some(&card));
    child.set_tooltip_text(Some(path.to_string_lossy().as_ref()));

    // Realce al situar el ratón encima (más fiable que :hover sobre hijos)
    let card_mov = card.clone();
    let motion = EventControllerMotion::new();
    motion.connect_enter(move |_, _, _| card_mov.add_css_class("hovered"));
    let card_mov2 = card.clone();
    motion.connect_leave(move |_| card_mov2.remove_css_class("hovered"));
    child.add_controller(motion);

    // Clic izquierdo: aplicar como fondo
    let ui2 = ui.clone();
    let path2 = path.to_path_buf();
    let click = GestureClick::new();
    click.connect_released(move |_, _, _, _| ui2.aplicar_fondo(&path2));
    child.add_controller(click);

    // Botón «aplicar» de la capa de acciones
    let ui3 = ui.clone();
    let path3 = path.to_path_buf();
    play.connect_clicked(move |_| ui3.aplicar_fondo(&path3));

    // Favorito
    let ui_fav = ui.clone();
    let path_fav = path.to_path_buf();
    fav.connect_clicked(move |btn| ui_fav.alternar_favorito(&path_fav, btn));

    // Vista previa
    let ui_prev = ui.clone();
    let path_prev = path.to_path_buf();
    previsualizar.connect_clicked(move |btn| {
        let parent = btn.root().and_downcast::<ApplicationWindow>();
        if let Some(win) = parent {
            abrir_preview(&ui_prev, &win, &path_prev);
        }
    });

    // Clic derecho o papelera: eliminar
    let ui4 = ui.clone();
    let path4 = path.to_path_buf();
    borrar.connect_clicked(move |_| ui4.pedir_eliminar(&path4));

    let ui5 = ui.clone();
    let path5 = path.to_path_buf();
    let click_der = GestureClick::new();
    click_der.set_button(3);
    click_der.connect_released(move |_, _, _, _| ui5.pedir_eliminar(&path5));
    child.add_controller(click_der);

    // Reordenar arrastrando (solo con orden manual)
    if Orden::desde_id(&ui.config.borrow().orden_biblioteca) == Orden::Manual {
        let origen = DragSource::new();
        origen.set_actions(gdk::DragAction::MOVE);
        {
            let ruta = path.to_string_lossy().to_string();
            origen.connect_prepare(move |_, _, _| {
                Some(gdk::ContentProvider::for_value(&ruta.to_value()))
            });
        }
        {
            let card_drag = card.clone();
            origen.connect_drag_begin(move |_, _| card_drag.add_css_class("dragging"));
            let card_soltar = card.clone();
            origen.connect_drag_end(move |_, _, _| card_soltar.remove_css_class("dragging"));
        }
        child.add_controller(origen);

        let destino = DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
        {
            let ui_d = ui.clone();
            let destino_path = path.to_path_buf();
            let child_d = child.clone();
            destino.connect_drop(move |_, valor, x, _| {
                let Ok(origen_ruta) = valor.get::<String>() else {
                    return false;
                };
                let destino_ruta = destino_path.to_string_lossy().to_string();
                if origen_ruta == destino_ruta {
                    return false;
                }
                let antes = x < f64::from(child_d.width()) / 2.0;
                ui_d.reordenar_manual(&origen_ruta, &destino_ruta, antes);
                true
            });
        }
        child.add_controller(destino);
    }

    ItemGrid {
        child,
        path: path.to_path_buf(),
        nombre: stem,
        es_video: core::es_video(path),
    }
}

// ── Vista previa ───────────────────────────────────────────────

/// Proceso mpv de una vista previa, con su socket IPC.
struct ProcesoPreview {
    hijo: RefCell<Option<Child>>,
    socket: PathBuf,
    incrustada: RefCell<Option<(RustConnection, u32)>>,
}

impl ProcesoPreview {
    fn detener(&self) {
        if let Some(mut c) = self.hijo.borrow_mut().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some((conn, ventana_x)) = self.incrustada.borrow_mut().take() {
            let _ = conn.destroy_window(ventana_x);
            let _ = conn.flush();
        }
        let _ = std::fs::remove_file(&self.socket);
    }

    fn enviar(&self, cmd: &str) {
        use std::io::Write as _;
        if let Ok(mut s) = std::os::unix::net::UnixStream::connect(&self.socket) {
            let _ = s.write_all(cmd.as_bytes());
            let _ = s.write_all(b"\n");
        }
    }

    /// Reposiciona y redimensiona la ventana X donde mpv dibuja el vídeo.
    fn encajar(&self, x: f64, y: f64, ancho: u32, alto: u32) {
        if let Some((conn, hija)) = self.incrustada.borrow().as_ref() {
            let _ = conn.configure_window(
                *hija,
                &ConfigureWindowAux::new()
                    .x(x.round() as i32)
                    .y(y.round() as i32)
                    .width(ancho)
                    .height(alto),
            );
            let _ = conn.flush();
        }
    }
}

/// Crea una ventana X hija dentro de `padre` para incrustar mpv.
fn crear_ventana_video(
    padre: u32,
    x: i16,
    y: i16,
    ancho: u16,
    alto: u16,
) -> Option<(RustConnection, u32)> {
    let (conn, _) = x11rb::connect(None).ok()?;
    let hija = conn.generate_id().ok()?;
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        hija,
        padre,
        x,
        y,
        ancho,
        alto,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new().background_pixel(0),
    )
    .ok()?;
    conn.map_window(hija).ok()?;
    conn.flush().ok()?;
    Some((conn, hija))
}

/// Abre una ventana sin bordes con el fondo reproduciéndose dentro (mpv vía X11).
fn abrir_preview(ui: &Rc<AppUi>, parent: &ApplicationWindow, path: &Path) {
    let ventana = gtk4::Window::builder()
        .title(core::nombre_base(path))
        .transient_for(parent)
        .default_width(920)
        .default_height(540)
        .decorated(false)
        .build();
    ventana.add_css_class("preview-shell");

    let socket = std::env::temp_dir().join(format!(
        "wallpaper-preview-{}-{}.sock",
        std::process::id(),
        aleatorio(1_000_000)
    ));
    let proceso = Rc::new(ProcesoPreview {
        hijo: RefCell::new(None),
        socket,
        incrustada: RefCell::new(None),
    });

    // Área de vídeo (aquí se incrusta mpv) y barra de controles debajo.
    // El vídeo va en una ventana X hija del área: si se incrustara sobre toda
    // la ventana, taparía los controles de GTK y no se podrían pulsar.
    let raiz = GtkBox::new(Orientation::Vertical, 0);
    raiz.set_vexpand(true);
    raiz.set_hexpand(true);

    let area = GtkBox::new(Orientation::Vertical, 0);
    area.set_vexpand(true);
    area.set_hexpand(true);
    raiz.append(&area);

    let barra = GtkBox::new(Orientation::Horizontal, 10);
    barra.add_css_class("preview-bar");
    barra.set_margin_start(14);
    barra.set_margin_end(14);
    barra.set_margin_top(10);
    barra.set_margin_bottom(12);

    let btn_pausa = Button::from_icon_name("media-playback-pause-symbolic");
    btn_pausa.add_css_class("hover-prev");
    btn_pausa.set_tooltip_text(Some("Pausar o reanudar la vista previa"));

    let btn_cerrar = Button::from_icon_name("window-close-symbolic");
    btn_cerrar.add_css_class("hover-prev");
    btn_cerrar.set_tooltip_text(Some("Cerrar vista previa (Esc)"));

    let btn_aplicar = boton_con_icono("btn-primary", "media-playback-start-symbolic", "Aplicar");

    // Información del archivo y edición de etiquetas (en la barra inferior)
    let mut partes: Vec<String> = Vec::new();
    if let Some(meta) = meta_de(path) {
        partes.push(meta.descripcion());
    }
    partes.push(core::peso_de(path));
    let info = Label::new(Some(&partes.join(" · ")));
    info.add_css_class("hero-sub");
    info.set_halign(Align::Start);
    info.set_hexpand(true);
    info.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    info.set_tooltip_text(Some(path.to_string_lossy().as_ref()));

    let caja_tags = GtkBox::new(Orientation::Horizontal, 8);
    caja_tags.set_valign(Align::Center);
    let etiqueta_lbl = Label::new(Some("Etiquetas:"));
    etiqueta_lbl.add_css_class("hero-sub");
    let entrada_tags = Entry::new();
    entrada_tags.set_placeholder_text(Some("separadas por comas"));
    entrada_tags.set_width_chars(18);
    entrada_tags.set_text(&etiquetas_de(&ui.config.borrow(), path).join(", "));
    caja_tags.append(&etiqueta_lbl);
    caja_tags.append(&entrada_tags);

    barra.append(&btn_pausa);
    barra.append(&info);
    barra.append(&caja_tags);
    barra.append(&btn_cerrar);
    barra.append(&btn_aplicar);

    {
        let ui2 = ui.clone();
        let path2 = path.to_path_buf();
        entrada_tags.connect_activate(move |entrada| {
            let mut vistas: Vec<String> = Vec::new();
            for t in entrada.text().split(',') {
                let t = t.trim();
                if !t.is_empty() && !vistas.iter().any(|v| v.eq_ignore_ascii_case(t)) {
                    vistas.push(t.to_string());
                }
            }
            let clave = path2.to_string_lossy().to_string();
            {
                let mut cfg = ui2.config.borrow_mut();
                if vistas.is_empty() {
                    cfg.etiquetas.remove(&clave);
                } else {
                    cfg.etiquetas.insert(clave, vistas);
                }
                cfg.guardar();
            }
            enviar_guardar_config(&ui2.config);
            ui2.toast("Etiquetas guardadas", "emblem-ok-symbolic");
            ui2.recargar();
        });
    }

    raiz.append(&barra);
    ventana.set_child(Some(&raiz));

    ventana.present();

    // Incrustación de mpv en una ventana X hija del área de vídeo. La
    // geometría se calcula en cada fotograma porque el tamaño real del
    // área no se conoce hasta que GTK hace el primer reparto de espacio.
    {
        let proceso2 = proceso.clone();
        let ui2 = ui.clone();
        let area2 = area.clone();
        let raiz2 = raiz.clone();
        let path2 = path.to_path_buf();
        let ultimo = Cell::new((0u32, 0u32));
        ventana.add_tick_callback(move |ventana, _| {
            let Some(surface) = ventana.surface() else {
                return glib::ControlFlow::Continue;
            };
            let Some(xid) = surface.downcast_ref::<X11Surface>().map(|s| s.xid()) else {
                return glib::ControlFlow::Continue;
            };
            let Some(rect) = area2.compute_bounds(&raiz2) else {
                return glib::ControlFlow::Continue;
            };
            let escala = f64::from(surface.scale_factor());
            let ancho = (f64::from(rect.width()) * escala).round() as u32;
            let alto = (f64::from(rect.height()) * escala).round() as u32;
            if ancho < 2 || alto < 2 {
                return glib::ControlFlow::Continue;
            }
            let x = f64::from(rect.x()) * escala;
            let y = f64::from(rect.y()) * escala;

            let ya_incrustada = proceso2.incrustada.borrow().is_some();
            if !ya_incrustada {
                if let Some((conn, hija)) = crear_ventana_video(
                    xid as u32,
                    x.round() as i16,
                    y.round() as i16,
                    ancho as u16,
                    alto as u16,
                ) {
                    let resultado = Command::new("mpv")
                        .args([
                            &format!("--wid={}", hija),
                            &format!("--input-ipc-server={}", proceso2.socket.display()),
                            "--no-audio",
                            "--loop-file=inf",
                            "--no-osc",
                            "--no-input-default-bindings",
                            "--no-terminal",
                            "--vo=gpu",
                            "--hwdec=auto-safe",
                            "--really-quiet",
                            "--focus-on=never",
                        ])
                        .arg(&path2)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn();
                    match resultado {
                        Ok(hijo) => *proceso2.hijo.borrow_mut() = Some(hijo),
                        Err(e) => ui2.toast(
                            &format!("No se pudo previsualizar: {}", e),
                            "dialog-error-symbolic",
                        ),
                    }
                    *proceso2.incrustada.borrow_mut() = Some((conn, hija));
                }
                ultimo.set((ancho, alto));
            } else if ultimo.get() != (ancho, alto) {
                ultimo.set((ancho, alto));
                proceso2.encajar(x, y, ancho, alto);
            }
            glib::ControlFlow::Continue
        });
    }

    // Controles
    {
        let proceso2 = proceso.clone();
        let pausado = std::cell::Cell::new(false);
        btn_pausa.connect_clicked(move |btn| {
            pausado.set(!pausado.get());
            let cmd = if pausado.get() {
                r#"{"command":["set_property","pause",true]}"#
            } else {
                r#"{"command":["set_property","pause",false]}"#
            };
            proceso2.enviar(cmd);
            btn.set_icon_name(if pausado.get() {
                "media-playback-start-symbolic"
            } else {
                "media-playback-pause-symbolic"
            });
        });
    }
    {
        let proceso2 = proceso.clone();
        let ventana2 = ventana.clone();
        btn_cerrar.connect_clicked(move |_| {
            proceso2.detener();
            ventana2.close();
        });
    }
    {
        let ui2 = ui.clone();
        let path2 = path.to_path_buf();
        let proceso2 = proceso.clone();
        let ventana2 = ventana.clone();
        btn_aplicar.connect_clicked(move |_| {
            proceso2.detener();
            ui2.aplicar_fondo(&path2);
            ventana2.close();
        });
    }
    {
        let proceso2 = proceso.clone();
        let ventana2 = ventana.clone();
        let tecla = EventControllerKey::new();
        tecla.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                proceso2.detener();
                ventana2.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        ventana.add_controller(tecla);
    }
    {
        let proceso2 = proceso.clone();
        ventana.connect_close_request(move |_| {
            proceso2.detener();
            glib::Propagation::Proceed
        });
    }
}

// ── Página de ajustes ──────────────────────────────────────────

fn titulo_seccion(icono_nombre: &str, texto: &str) -> GtkBox {
    let caja = GtkBox::new(Orientation::Horizontal, 10);
    caja.set_margin_top(26);
    caja.set_margin_bottom(4);
    let tile = GtkBox::new(Orientation::Vertical, 0);
    tile.add_css_class("set-tile");
    tile.set_valign(Align::Center);
    tile.append(&icono(icono_nombre, 18));
    caja.append(&tile);
    let l = Label::new(Some(texto));
    l.add_css_class("seccion-titulo");
    l.set_halign(Align::Start);
    l.set_valign(Align::Center);
    caja.append(&l);
    caja
}

fn fila_ajuste(
    icono_nombre: &str,
    titulo: &str,
    sub: &str,
    widget: &impl IsA<gtk4::Widget>,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.add_css_class("settings-row");

    let hbox = GtkBox::new(Orientation::Horizontal, 14);
    hbox.set_margin_start(16);
    hbox.set_margin_end(16);
    hbox.set_margin_top(12);
    hbox.set_margin_bottom(12);

    let tile = GtkBox::new(Orientation::Vertical, 0);
    tile.add_css_class("set-tile");
    tile.set_valign(Align::Start);
    tile.append(&icono(icono_nombre, 18));
    hbox.append(&tile);

    let vbox = GtkBox::new(Orientation::Vertical, 3);
    vbox.set_hexpand(true);
    vbox.set_valign(Align::Center);
    let l1 = Label::new(Some(titulo));
    l1.set_halign(Align::Start);
    l1.add_css_class("titulo");
    vbox.append(&l1);
    let l2 = Label::new(Some(sub));
    l2.set_halign(Align::Start);
    l2.set_wrap(true);
    l2.add_css_class("sub");
    vbox.append(&l2);
    hbox.append(&vbox);

    hbox.append(widget);
    row.set_child(Some(&hbox));
    row
}

fn lista_ajustes() -> ListBox {
    let lista = ListBox::new();
    lista.add_css_class("settings-card");
    lista.set_selection_mode(SelectionMode::None);
    lista
}

/// Separador horizontal de la tarjeta de información.
fn separador() -> gtk4::Separator {
    let sep = gtk4::Separator::new(Orientation::Horizontal);
    sep.set_margin_start(16);
    sep.set_margin_end(16);
    sep
}

/// Fila clave/valor de la tarjeta de información; el valor se actualiza en caliente.
fn fila_info(clave: &str, valor: &Label) -> GtkBox {
    let fila = GtkBox::new(Orientation::Horizontal, 12);
    fila.set_margin_start(18);
    fila.set_margin_end(18);
    fila.set_margin_top(12);
    fila.set_margin_bottom(12);
    let lk = Label::new(Some(clave));
    lk.add_css_class("notas");
    lk.set_halign(Align::Start);
    fila.append(&lk);
    valor.set_halign(Align::End);
    valor.set_xalign(1.0);
    valor.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    valor.set_max_width_chars(46);
    valor.add_css_class("notas");
    fila.append(valor);
    fila
}

fn construir_ajustes(ui: &Rc<AppUi>) -> GtkBox {
    let cont = GtkBox::new(Orientation::Vertical, 0);
    cont.set_halign(Align::Center);
    cont.set_margin_top(10);
    cont.set_margin_bottom(48);
    cont.set_size_request(780, -1);

    // ── Sistema ───────────────────────────────────────────────
    cont.append(&titulo_seccion("preferences-system-symbolic", "SISTEMA"));
    let lista_sistema = lista_ajustes();
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().autostart);
        sw.set_valign(Align::Center);
        let row = fila_ajuste(
            "system-run-symbolic",
            "Iniciar con la sesión",
            "Ejecutar el fondo animado al iniciar sesión",
            &sw,
        );
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            let activo = s.is_active();
            ui2.config.borrow_mut().autostart = activo;
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
            if let Ok(exe) = std::env::current_exe() {
                let menu_dir = exe.parent().unwrap_or(Path::new("."));
                let daemon = menu_dir.join("wallpaper-daemon");
                core::aplicar_autostart(activo, &daemon);
            }
        });
        lista_sistema.append(&row);
    }
    cont.append(&lista_sistema);

    // ── Biblioteca ────────────────────────────────────────────
    cont.append(&titulo_seccion("folder-symbolic", "BIBLIOTECA"));
    let lista_bib = lista_ajustes();
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().enlazar_al_importar);
        sw.set_valign(Align::Center);
        let row = fila_ajuste(
            "edit-copy-symbolic",
            "Enlazar en vez de copiar",
            "Al importar, crea un enlace simbólico en lugar de duplicar el archivo",
            &sw,
        );
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            ui2.config.borrow_mut().enlazar_al_importar = s.is_active();
            ui2.config.borrow().guardar();
        });
        lista_bib.append(&row);
    }
    cont.append(&lista_bib);

    // ── Reproducción ──────────────────────────────────────────
    cont.append(&titulo_seccion("video-x-generic-symbolic", "REPRODUCCIÓN"));
    let lista_repro = lista_ajustes();
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().pausa_fullscreen);
        sw.set_valign(Align::Center);
        let row = fila_ajuste(
            "view-fullscreen-symbolic",
            "Pausar en pantalla completa",
            "Útil para jugar o ver vídeos sin consumir recursos extra",
            &sw,
        );
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            ui2.config.borrow_mut().pausa_fullscreen = s.is_active();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        lista_repro.append(&row);
    }
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().pausa_bateria);
        sw.set_valign(Align::Center);
        let row = fila_ajuste(
            "battery-good-symbolic",
            "Pausar con batería",
            "Ahorra energía en portátiles",
            &sw,
        );
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            ui2.config.borrow_mut().pausa_bateria = s.is_active();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        lista_repro.append(&row);
    }
    {
        let chips = GtkBox::new(Orientation::Horizontal, 6);
        chips.add_css_class("chips");
        chips.set_valign(Align::Center);
        let opciones: [(u32, &str); 4] = [(0, "Sin límite"), (24, "24"), (30, "30"), (60, "60")];
        let actual = ui.config.borrow().fps_limit;
        let mut botones: Vec<(u32, ToggleButton)> = Vec::new();
        for (valor, texto) in opciones {
            let b = ToggleButton::with_label(texto);
            if let Some((_, anterior)) = botones.last() {
                b.set_group(Some(anterior));
            }
            chips.append(&b);
            botones.push((valor, b));
        }
        let elegido = botones.iter().position(|(v, _)| *v == actual).unwrap_or(0);
        botones[elegido].1.set_active(true);
        for (valor, b) in &botones {
            let ui2 = ui.clone();
            let valor = *valor;
            b.connect_toggled(move |btn| {
                if btn.is_active() {
                    ui2.config.borrow_mut().fps_limit = valor;
                    ui2.config.borrow().guardar();
                    enviar_guardar_config(&ui2.config);
                }
            });
        }
        let row = fila_ajuste(
            "view-refresh-symbolic",
            "Límite de FPS",
            "Reduce el uso de GPU si el equipo va justito",
            &chips,
        );
        lista_repro.append(&row);
    }
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().pausa_app);
        sw.set_valign(Align::Center);
        let row = fila_ajuste(
            "focus-windows-symbolic",
            "Pausar con app en primer plano",
            "El fondo se congela mientras usas una aplicación y se reanuda al volver al escritorio",
            &sw,
        );
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            ui2.config.borrow_mut().pausa_app = s.is_active();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        lista_repro.append(&row);
    }
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().pausa_workspace);
        sw.set_valign(Align::Center);
        let row = fila_ajuste(
            "preferences-system-windows-symbolic",
            "Pausar al cambiar de espacio de trabajo",
            "Congela el fondo un instante mientras cambias de escritorio virtual",
            &sw,
        );
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            ui2.config.borrow_mut().pausa_workspace = s.is_active();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        lista_repro.append(&row);
    }
    cont.append(&lista_repro);

    // ── Pausas inteligentes ───────────────────────────────────
    cont.append(&titulo_seccion("changes-prevent-symbolic", "PAUSAS"));
    let lista_pausas = lista_ajustes();
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().pausa_bloqueo);
        sw.set_valign(Align::Center);
        let row = fila_ajuste(
            "system-lock-screen-symbolic",
            "Pausar con pantalla bloqueada",
            "El fondo se congela mientras la sesión está bloqueada y se reanuda al volver",
            &sw,
        );
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            ui2.config.borrow_mut().pausa_bloqueo = s.is_active();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        lista_pausas.append(&row);
    }
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().pausa_inactividad);
        sw.set_valign(Align::Center);
        let spin = SpinButton::with_range(1.0, 240.0, 1.0);
        spin.set_value(f64::from(ui.config.borrow().pausa_inactividad_minutos));
        spin.set_valign(Align::Center);
        let caja = GtkBox::new(Orientation::Horizontal, 8);
        caja.append(&spin);
        let unidad = Label::new(Some("min"));
        unidad.add_css_class("notas");
        unidad.set_valign(Align::Center);
        caja.append(&unidad);
        caja.append(&sw);
        let row = fila_ajuste(
            "preferences-system-time-symbolic",
            "Pausar por inactividad",
            "Congela el fondo tras un tiempo sin teclado ni ratón",
            &caja,
        );
        spin.set_sensitive(sw.is_active());
        let spin2 = spin.clone();
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            spin2.set_sensitive(s.is_active());
            ui2.config.borrow_mut().pausa_inactividad = s.is_active();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        let ui3 = ui.clone();
        spin.connect_value_changed(move |s| {
            ui3.config.borrow_mut().pausa_inactividad_minutos = s.value() as u32;
            ui3.config.borrow().guardar();
            enviar_guardar_config(&ui3.config);
        });
        lista_pausas.append(&row);
    }
    cont.append(&lista_pausas);

    // ── Automático ────────────────────────────────────────────
    cont.append(&titulo_seccion(
        "media-playlist-shuffle-symbolic",
        "AUTOMÁTICO",
    ));
    let lista_auto = lista_ajustes();
    let sw_slideshow = Switch::new();
    let row_intervalo;
    {
        sw_slideshow.set_active(ui.config.borrow().slideshow);
        sw_slideshow.set_valign(Align::Center);
        let row = fila_ajuste(
            "media-playlist-shuffle-symbolic",
            "Cambio automático de fondo",
            "Rota entre tus fondos periódicamente",
            &sw_slideshow,
        );
        lista_auto.append(&row);
    }
    {
        let sw = Switch::new();
        sw.set_active(ui.config.borrow().slideshow_aleatorio);
        sw.set_valign(Align::Center);
        let row = fila_ajuste(
            "media-playlist-consecutive-symbolic",
            "Orden aleatorio",
            "Elige el siguiente fondo al azar en vez de seguir el orden de la lista",
            &sw,
        );
        let ui2 = ui.clone();
        sw.connect_active_notify(move |s| {
            ui2.config.borrow_mut().slideshow_aleatorio = s.is_active();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        lista_auto.append(&row);
    }
    {
        let spin = SpinButton::with_range(1.0, 1440.0, 1.0);
        spin.set_value(ui.config.borrow().slideshow_minutos as f64);
        spin.set_valign(Align::Center);
        spin.set_width_chars(5);

        let hbox = GtkBox::new(Orientation::Horizontal, 8);
        hbox.set_valign(Align::Center);
        hbox.append(&spin);
        hbox.append(&chip("MIN"));

        row_intervalo = fila_ajuste(
            "preferences-system-time-symbolic",
            "Intervalo de cambio",
            "Cada cuántos minutos cambia el fondo",
            &hbox,
        );
        row_intervalo.set_sensitive(ui.config.borrow().slideshow);

        let ui2 = ui.clone();
        sw_slideshow.connect_active_notify(move |s| {
            ui2.config.borrow_mut().slideshow = s.is_active();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        let row_sens = row_intervalo.clone();
        sw_slideshow.connect_active_notify(move |s| row_sens.set_sensitive(s.is_active()));

        let ui3 = ui.clone();
        spin.connect_value_changed(move |s| {
            ui3.config.borrow_mut().slideshow_minutos = s.value() as u32;
            ui3.config.borrow().guardar();
            enviar_guardar_config(&ui3.config);
        });
        lista_auto.append(&row_intervalo);
    }
    cont.append(&lista_auto);

    // ── Atajos ────────────────────────────────────────────────
    cont.append(&titulo_seccion("input-keyboard-symbolic", "ATAJOS"));
    let lista_atajos = lista_ajustes();
    {
        let descripciones: Vec<&str> = core::HOTKEYS.iter().map(|(_, d)| *d).collect();
        let modelo = StringList::new(&descripciones);
        let drop = DropDown::new(Some(modelo), None::<gtk4::Expression>);
        drop.set_valign(Align::Center);
        let actual = ui.config.borrow().hotkey.clone();
        let idx = core::HOTKEYS
            .iter()
            .position(|(id, _)| *id == actual)
            .unwrap_or(0) as u32;
        drop.set_selected(idx);

        let row = fila_ajuste(
            "preferences-desktop-keyboard-shortcuts-symbolic",
            "Atajo para abrir el menú",
            "El servidor intenta registrarlo; si está ocupado usa otro alternativo",
            &drop,
        );
        let ui2 = ui.clone();
        drop.connect_selected_notify(move |d| {
            let idx = d.selected() as usize;
            let Some((id, _)) = core::HOTKEYS.get(idx) else {
                return;
            };
            ui2.config.borrow_mut().hotkey = (*id).to_string();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        lista_atajos.append(&row);
    }
    {
        let descripciones: Vec<&str> = core::HOTKEYS.iter().map(|(_, d)| *d).collect();
        let modelo = StringList::new(&descripciones);
        let drop = DropDown::new(Some(modelo), None::<gtk4::Expression>);
        drop.set_valign(Align::Center);
        let actual = ui.config.borrow().hotkey_siguiente.clone();
        let idx = core::HOTKEYS
            .iter()
            .position(|(id, _)| *id == actual)
            .unwrap_or(0) as u32;
        drop.set_selected(idx);

        let row = fila_ajuste(
            "media-skip-forward-symbolic",
            "Atajo para siguiente fondo",
            "Cambia de fondo sin abrir el menú",
            &drop,
        );
        let ui2 = ui.clone();
        drop.connect_selected_notify(move |d| {
            let idx = d.selected() as usize;
            let Some((id, _)) = core::HOTKEYS.get(idx) else {
                return;
            };
            ui2.config.borrow_mut().hotkey_siguiente = (*id).to_string();
            ui2.config.borrow().guardar();
            enviar_guardar_config(&ui2.config);
        });
        lista_atajos.append(&row);
    }
    cont.append(&lista_atajos);

    // ── Información ───────────────────────────────────────────
    cont.append(&titulo_seccion(
        "dialog-information-symbolic",
        "INFORMACIÓN",
    ));
    let info = GtkBox::new(Orientation::Vertical, 8);
    info.add_css_class("settings-card");
    info.set_margin_top(8);
    let dir_txt = ui.dir.to_string_lossy().to_string();

    let lv_version = Label::new(Some(env!("CARGO_PKG_VERSION")));
    info.append(&fila_info("Versión", &lv_version));
    info.append(&separador());
    info.append(&fila_info("Atajo global", &ui.lbl_info_hotkey));
    info.append(&separador());
    info.append(&fila_info("Atajo siguiente", &ui.lbl_info_hotkey_siguiente));
    info.append(&separador());
    info.append(&fila_info("Servidor", &ui.lbl_info_servidor));
    info.append(&separador());
    let lv_biblio = Label::new(Some(&dir_txt));
    info.append(&fila_info("Biblioteca", &lv_biblio));
    cont.append(&info);

    let nota = Label::new(Some(
        "Los cambios se aplican al instante. El servidor debe estar en ejecución.",
    ));
    nota.add_css_class("notas");
    nota.set_margin_top(20);
    nota.set_halign(Align::Center);
    cont.append(&nota);

    cont
}

// ── Página de perfiles ─────────────────────────────────────────

/// Página «Perfiles»: guardar la personalización actual (temas, fastfetch,
/// kitty) y aplicar o eliminar perfiles guardados.
fn construir_perfiles(ui: &Rc<AppUi>) -> GtkBox {
    let cont = GtkBox::new(Orientation::Vertical, 0);
    cont.set_halign(Align::Center);
    cont.set_margin_top(10);
    cont.set_margin_bottom(48);
    cont.set_size_request(780, -1);

    cont.append(&titulo_seccion(
        "preferences-desktop-theme-symbolic",
        "PERFILES",
    ));
    let card_guardar = lista_ajustes();
    {
        let caja = GtkBox::new(Orientation::Horizontal, 8);
        caja.set_valign(Align::Center);
        let entrada = Entry::new();
        entrada.set_placeholder_text(Some("Nombre del perfil"));
        entrada.set_width_chars(18);
        let btn = boton_con_icono("btn-primary", "document-save-symbolic", "Guardar");
        caja.append(&entrada);
        caja.append(&btn);

        let ui2 = ui.clone();
        let entrada2 = entrada.clone();
        btn.connect_clicked(move |_| guardar_desde_ui(&ui2, &entrada2));
        let ui3 = ui.clone();
        entrada.connect_activate(move |e| guardar_desde_ui(&ui3, e));

        let row = fila_ajuste(
            "document-save-symbolic",
            "Guardar perfil actual",
            "Incluye tus temas (GTK, ventanas y Cinnamon), la configuración de fastfetch y la de kitty",
            &caja,
        );
        card_guardar.append(&row);
    }
    cont.append(&card_guardar);

    cont.append(&titulo_seccion("user-home-symbolic", "GUARDADOS"));
    cont.append(&ui.lista_perfiles);
    refrescar_perfiles(ui);
    cont
}

/// Reconstruye la lista de perfiles guardados.
fn refrescar_perfiles(ui: &Rc<AppUi>) {
    while let Some(hijo) = ui.lista_perfiles.first_child() {
        ui.lista_perfiles.remove(&hijo);
    }
    let perfiles = core::listar_perfiles();
    if perfiles.is_empty() {
        let row = ListBoxRow::new();
        row.set_activatable(false);
        row.add_css_class("settings-row");
        let l = Label::new(Some("Aún no hay perfiles guardados"));
        l.add_css_class("sub");
        l.set_halign(Align::Start);
        l.set_margin_top(14);
        l.set_margin_bottom(14);
        l.set_margin_start(16);
        l.set_margin_end(16);
        row.set_child(Some(&l));
        ui.lista_perfiles.append(&row);
        return;
    }
    for perfil in perfiles {
        ui.lista_perfiles.append(&fila_perfil(ui, &perfil));
    }
}

/// Fila de un perfil con su resumen y los botones Aplicar / Eliminar.
fn fila_perfil(ui: &Rc<AppUi>, perfil: &core::Perfil) -> ListBoxRow {
    let botones = GtkBox::new(Orientation::Horizontal, 8);
    botones.set_valign(Align::Center);
    let btn_aplicar = boton_con_icono("btn-primary", "emblem-ok-symbolic", "Aplicar");
    let btn_eliminar = boton_con_icono("btn-danger", "user-trash-symbolic", "");
    btn_eliminar.set_tooltip_text(Some("Eliminar el perfil"));
    botones.append(&btn_aplicar);
    botones.append(&btn_eliminar);

    let mut partes: Vec<String> = Vec::new();
    if let Some(v) = &perfil.gtk {
        partes.push(v.clone());
    }
    if let Some(v) = &perfil.wm {
        partes.push(format!("Ventanas: {v}"));
    }
    if let Some(v) = &perfil.cinnamon {
        partes.push(format!("Cinnamon: {v}"));
    }
    if perfil.fastfetch.is_some() {
        partes.push("fastfetch".to_string());
    }
    if perfil.kitty.is_some() {
        partes.push("kitty".to_string());
    }
    let resumen = if partes.is_empty() {
        "Sin contenido".to_string()
    } else {
        partes.join(" · ")
    };

    let row = fila_ajuste(
        "preferences-desktop-theme-symbolic",
        &perfil.nombre,
        &resumen,
        &botones,
    );

    let ui2 = ui.clone();
    let p2 = perfil.clone();
    btn_aplicar.connect_clicked(move |_| match core::aplicar_perfil(&p2) {
        Ok(()) => ui2.toast(
            &format!("Perfil «{}» aplicado", p2.nombre),
            "emblem-ok-symbolic",
        ),
        Err(e) => ui2.toast(
            &format!("No se pudo aplicar: {e}"),
            "dialog-warning-symbolic",
        ),
    });

    let ui3 = ui.clone();
    let p3 = perfil.clone();
    btn_eliminar.connect_clicked(move |_| {
        let dialogo = gtk4::AlertDialog::builder()
            .message("¿Eliminar el perfil?")
            .detail(format!("«{}» se borrará de forma permanente", p3.nombre))
            .buttons(["Cancelar", "Eliminar"])
            .build();
        let ventana = ui3.lightbox.root().and_downcast::<gtk4::Window>();
        let ui4 = ui3.clone();
        let p4 = p3.clone();
        dialogo.choose(ventana.as_ref(), None::<&gio::Cancellable>, move |res| {
            let Ok(boton) = res else {
                return;
            };
            if boton != 1 {
                return;
            }
            match core::eliminar_perfil(&p4) {
                Ok(()) => {
                    refrescar_perfiles(&ui4);
                    ui4.toast(
                        &format!("Perfil «{}» eliminado", p4.nombre),
                        "user-trash-symbolic",
                    );
                }
                Err(e) => ui4.toast(
                    &format!("No se pudo eliminar: {e}"),
                    "dialog-warning-symbolic",
                ),
            }
        });
    });

    row
}

/// Valida el nombre y guarda; si ya existe un perfil con ese nombre, pregunta.
fn guardar_desde_ui(ui: &Rc<AppUi>, entrada: &Entry) {
    let nombre = entrada.text().trim().to_string();
    if nombre.is_empty() {
        ui.toast(
            "Escribe un nombre para el perfil",
            "dialog-warning-symbolic",
        );
        return;
    }
    let existe = core::listar_perfiles()
        .iter()
        .any(|p| core::slug(&p.nombre) == core::slug(&nombre));
    if !existe {
        guardar_perfil_ahora(ui, entrada, &nombre);
        return;
    }
    let dialogo = gtk4::AlertDialog::builder()
        .message("Ya existe un perfil con ese nombre")
        .detail("¿Quieres reemplazarlo con la configuración actual?")
        .buttons(["Cancelar", "Reemplazar"])
        .build();
    let ventana = ui.lightbox.root().and_downcast::<gtk4::Window>();
    let ui2 = ui.clone();
    let entrada2 = entrada.clone();
    dialogo.choose(ventana.as_ref(), None::<&gio::Cancellable>, move |res| {
        let Ok(boton) = res else {
            return;
        };
        if boton == 1 {
            guardar_perfil_ahora(&ui2, &entrada2, &nombre);
        }
    });
}

fn guardar_perfil_ahora(ui: &Rc<AppUi>, entrada: &Entry, nombre: &str) {
    match core::guardar_perfil(nombre) {
        Ok(_) => {
            entrada.set_text("");
            refrescar_perfiles(ui);
            ui.toast(&format!("Perfil «{nombre}» guardado"), "emblem-ok-symbolic");
        }
        Err(e) => ui.toast(
            &format!("No se pudo guardar: {e}"),
            "dialog-warning-symbolic",
        ),
    }
}

// ── Construcción de la ventana ─────────────────────────────────

fn build_ui(app: &Application) {
    if let Some(display) = gdk::Display::default() {
        let iconos = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        gtk4::IconTheme::for_display(&display).add_search_path(&iconos);
    }
    gtk4::Window::set_default_icon_name("wallpaper-animado");
    let config = Config::cargar();
    let dir = core::wallpapers_dir();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Azura")
        .default_width(1240)
        .default_height(820)
        .build();
    window.set_size_request(980, 640);

    // ── Cabecera ──────────────────────────────────────────────
    let header = HeaderBar::new();
    let titulo = Label::new(Some("Azura"));
    header.set_title_widget(Some(&titulo));
    window.set_titlebar(Some(&header));

    // ── Lateral ───────────────────────────────────────────────
    let sidebar = GtkBox::new(Orientation::Vertical, 0);
    sidebar.add_css_class("sidebar");
    sidebar.set_size_request(252, -1);

    let marca = GtkBox::new(Orientation::Horizontal, 12);
    marca.set_margin_bottom(22);
    let logo = Picture::for_filename(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon.png"));
    logo.set_content_fit(ContentFit::Contain);
    logo.set_size_request(44, 44);
    logo.set_valign(Align::Center);
    marca.append(&logo);
    let marca_txt = GtkBox::new(Orientation::Vertical, 2);
    let m1 = Label::new(Some("Azura"));
    m1.add_css_class("brand-nombre");
    m1.set_halign(Align::Start);
    let m2 = Label::new(Some("GESTOR DE FONDOS"));
    m2.add_css_class("brand-sub");
    m2.set_halign(Align::Start);
    marca_txt.append(&m1);
    marca_txt.append(&m2);
    marca.append(&marca_txt);
    sidebar.append(&marca);

    let nav_biblio = ToggleButton::new();
    nav_biblio.add_css_class("nav-btn");
    nav_biblio.set_halign(Align::Fill);
    {
        let c = GtkBox::new(Orientation::Horizontal, 12);
        c.append(&icono("view-grid-symbolic", 18));
        c.append(&Label::new(Some("Biblioteca")));
        nav_biblio.set_child(Some(&c));
    }
    let nav_ajustes = ToggleButton::new();
    nav_ajustes.add_css_class("nav-btn");
    nav_ajustes.set_halign(Align::Fill);
    nav_ajustes.set_margin_top(4);
    {
        let c = GtkBox::new(Orientation::Horizontal, 12);
        c.append(&icono("preferences-system-symbolic", 18));
        c.append(&Label::new(Some("Ajustes")));
        nav_ajustes.set_child(Some(&c));
    }
    nav_ajustes.set_group(Some(&nav_biblio));
    nav_biblio.set_active(true);
    sidebar.append(&nav_biblio);
    sidebar.append(&nav_ajustes);
    let nav_perfiles = ToggleButton::new();
    nav_perfiles.add_css_class("nav-btn");
    nav_perfiles.set_halign(Align::Fill);
    nav_perfiles.set_margin_top(4);
    {
        let c = GtkBox::new(Orientation::Horizontal, 12);
        c.append(&icono("preferences-desktop-theme-symbolic", 18));
        c.append(&Label::new(Some("Perfiles")));
        nav_perfiles.set_child(Some(&c));
    }
    nav_perfiles.set_group(Some(&nav_biblio));
    sidebar.append(&nav_perfiles);

    let empuje = GtkBox::new(Orientation::Vertical, 0);
    empuje.set_vexpand(true);
    sidebar.append(&empuje);

    let info = GtkBox::new(Orientation::Vertical, 10);
    info.add_css_class("info-card");
    info.set_margin_bottom(12);
    let fila_estado = GtkBox::new(Orientation::Horizontal, 8);
    let dot = Label::new(Some("●"));
    dot.add_css_class("dot-on");
    let txt = Label::new(Some("Comprobando…"));
    txt.add_css_class("info-label");
    fila_estado.append(&dot);
    fila_estado.append(&txt);
    info.append(&fila_estado);
    let cuenta = Label::new(Some("— fondos en la biblioteca"));
    cuenta.add_css_class("info-label");
    cuenta.set_halign(Align::Start);
    info.append(&cuenta);

    let lbl_hotkey = Label::new(Some("Atajo: —"));
    lbl_hotkey.add_css_class("info-label");
    lbl_hotkey.set_halign(Align::Start);
    info.append(&lbl_hotkey);

    let lbl_deps = Label::new(Some(""));
    lbl_deps.add_css_class("deps-warn");
    lbl_deps.set_halign(Align::Start);
    lbl_deps.set_wrap(true);
    lbl_deps.set_visible(false);
    info.append(&lbl_deps);
    sidebar.append(&info);

    let btn_iniciar = boton_con_icono("btn-ghost", "system-run-symbolic", "Iniciar servidor");
    btn_iniciar.set_halign(Align::Fill);
    btn_iniciar.set_visible(false);
    btn_iniciar.set_margin_bottom(8);
    sidebar.append(&btn_iniciar);

    let btn_add = boton_con_icono("btn-primary", "list-add-symbolic", "Añadir fondo");
    btn_add.set_halign(Align::Fill);
    sidebar.append(&btn_add);

    // ── Página: biblioteca ────────────────────────────────────
    let pagina_bib = GtkBox::new(Orientation::Vertical, 0);
    pagina_bib.set_margin_top(18);
    pagina_bib.set_margin_bottom(18);
    pagina_bib.set_margin_start(22);
    pagina_bib.set_margin_end(22);

    // Destacado
    let hero = Overlay::new();
    hero.add_css_class("hero");
    hero.set_size_request(-1, 208);

    let hero_pic = Picture::new();
    hero_pic.add_css_class("hero-pic");
    hero_pic.set_content_fit(ContentFit::Cover);
    hero_pic.set_can_shrink(true);
    hero_pic.set_halign(Align::Fill);
    hero_pic.set_valign(Align::Fill);
    hero.set_child(Some(&hero_pic));

    let scrim = GtkBox::new(Orientation::Vertical, 0);
    scrim.add_css_class("hero-scrim");
    scrim.set_halign(Align::Fill);
    scrim.set_valign(Align::Fill);
    scrim.set_hexpand(true);
    scrim.set_vexpand(true);
    hero.add_overlay(&scrim);

    let hero_contenido = GtkBox::new(Orientation::Horizontal, 16);
    hero_contenido.set_halign(Align::Fill);
    hero_contenido.set_valign(Align::End);
    hero_contenido.set_margin_start(22);
    hero_contenido.set_margin_end(22);
    hero_contenido.set_margin_bottom(20);
    hero_contenido.set_margin_top(16);

    let hero_izq = GtkBox::new(Orientation::Vertical, 8);
    hero_izq.set_hexpand(true);
    let hero_badge = GtkBox::new(Orientation::Horizontal, 8);
    hero_badge.set_halign(Align::Start);
    hero_badge.append(&icono("media-playback-start-symbolic", 12));
    let vivo = Label::new(Some("EN REPRODUCCIÓN"));
    vivo.add_css_class("chip");
    vivo.add_css_class("chip-vivo");
    hero_badge.append(&vivo);
    hero_izq.append(&hero_badge);

    let hero_titulo = Label::new(Some("—"));
    hero_titulo.add_css_class("hero-titulo");
    hero_titulo.set_halign(Align::Start);
    hero_titulo.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    hero_titulo.set_max_width_chars(40);
    hero_izq.append(&hero_titulo);

    let hero_sub = Label::new(Some(""));
    hero_sub.add_css_class("hero-sub");
    hero_sub.set_halign(Align::Start);
    hero_sub.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    hero_sub.set_max_width_chars(70);
    hero_izq.append(&hero_sub);

    let hero_chips = GtkBox::new(Orientation::Horizontal, 6);
    hero_chips.set_halign(Align::Start);
    hero_izq.append(&hero_chips);
    hero_contenido.append(&hero_izq);

    let hero_acciones = GtkBox::new(Orientation::Horizontal, 10);
    hero_acciones.set_valign(Align::End);

    let hero_monitor = GtkBox::new(Orientation::Horizontal, 8);
    hero_monitor.set_valign(Align::Center);
    hero_monitor.set_visible(false);
    let etiqueta_monitor = Label::new(Some("Pantalla"));
    etiqueta_monitor.add_css_class("hero-sub");
    hero_monitor.append(&etiqueta_monitor);
    let monitor_dd = DropDown::from_strings(&["Todos"]);
    monitor_dd.set_valign(Align::Center);
    hero_monitor.append(&monitor_dd);
    hero_acciones.append(&hero_monitor);

    let btn_pausa = boton_con_icono("btn-hero", "media-playback-pause-symbolic", "Pausar");
    btn_pausa.set_sensitive(false);
    let btn_quitar = boton_con_icono("btn-hero", "user-trash-symbolic", "Quitar");
    btn_quitar.set_sensitive(false);
    let btn_aleatorio = boton_con_icono("btn-hero", "media-playlist-shuffle-symbolic", "Aleatorio");
    let btn_carpeta = boton_con_icono("btn-hero", "folder-download-symbolic", "Abrir carpeta");
    hero_acciones.append(&btn_pausa);
    hero_acciones.append(&btn_quitar);
    hero_acciones.append(&btn_aleatorio);
    hero_acciones.append(&btn_carpeta);
    hero_contenido.append(&hero_acciones);
    hero.add_overlay(&hero_contenido);
    pagina_bib.append(&hero);

    // Barra de herramientas
    let barra = GtkBox::new(Orientation::Horizontal, 12);
    barra.set_margin_top(18);
    barra.set_margin_bottom(12);
    let search = SearchEntry::new();
    search.add_css_class("search");
    search.set_placeholder_text(Some("Buscar en la biblioteca…"));
    search.set_width_chars(26);
    search.set_hexpand(true);
    barra.append(&search);

    let chips = GtkBox::new(Orientation::Horizontal, 6);
    chips.add_css_class("chips");
    let chip_todo = ToggleButton::with_label("Todo");
    let chip_video = ToggleButton::with_label("Vídeos");
    chip_video.set_group(Some(&chip_todo));
    let chip_imagen = ToggleButton::with_label("Imágenes");
    chip_imagen.set_group(Some(&chip_todo));
    let chip_favoritos = ToggleButton::with_label("Favoritos");
    chip_favoritos.set_group(Some(&chip_todo));
    chip_todo.set_active(true);
    chips.append(&chip_todo);
    chips.append(&chip_video);
    chips.append(&chip_imagen);
    chips.append(&chip_favoritos);
    barra.append(&chips);

    let orden_dd = DropDown::from_strings(&[
        "Nombre A-Z",
        "Nombre Z-A",
        "Más recientes",
        "Tamaño",
        "Manual (arrastrar)",
    ]);
    orden_dd.set_valign(Align::Center);
    orden_dd.set_tooltip_text(Some(
        "Ordenar la biblioteca. En «Manual» puedes arrastrar las tarjetas para reordenarlas",
    ));
    let orden_actual = Orden::desde_id(&config.orden_biblioteca);
    orden_dd.set_selected(match orden_actual {
        Orden::NombreAz => 0,
        Orden::NombreZa => 1,
        Orden::Fecha => 2,
        Orden::Tamano => 3,
        Orden::Manual => 4,
    });
    barra.append(&orden_dd);

    let contador = Label::new(Some("0 fondos"));
    contador.add_css_class("contador");
    contador.set_valign(Align::Center);
    barra.append(&contador);
    pagina_bib.append(&barra);

    // Rejilla + estado vacío
    let paginas_biblio = Stack::new();
    paginas_biblio.set_transition_type(StackTransitionType::Crossfade);
    paginas_biblio.set_transition_duration(180);
    paginas_biblio.set_vexpand(true);

    let flow = FlowBox::new();
    flow.set_valign(Align::Start);
    flow.set_row_spacing(10);
    flow.set_column_spacing(10);
    flow.set_max_children_per_line(4);
    flow.set_min_children_per_line(1);
    flow.set_selection_mode(SelectionMode::None);
    flow.set_homogeneous(true);

    let scroll = ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .child(&flow)
        .build();
    scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);
    paginas_biblio.add_titled(&scroll, Some("grid"), "Biblioteca");

    let vacio = GtkBox::new(Orientation::Vertical, 12);
    vacio.set_valign(Align::Center);
    vacio.set_halign(Align::Center);
    let vacio_icono = icono("preferences-desktop-wallpaper-symbolic", 72);
    vacio_icono.add_css_class("empty-icon");
    vacio.append(&vacio_icono);
    let vacio_titulo = Label::new(Some("Tu biblioteca está vacía"));
    vacio_titulo.add_css_class("empty-titulo");
    vacio.append(&vacio_titulo);
    let vacio_sub = Label::new(Some("Añade vídeos o imágenes, o arrastra archivos aquí"));
    vacio_sub.add_css_class("empty-sub");
    vacio.append(&vacio_sub);
    let vacio_btn = boton_con_icono("btn-ghost", "list-add-symbolic", "Añadir fondo");
    vacio_btn.set_margin_top(8);
    vacio.append(&vacio_btn);
    paginas_biblio.add_titled(&vacio, Some("vacio"), "Vacío");

    // Estado "sin coincidencias" (búsqueda o filtro sin resultados)
    let sin = GtkBox::new(Orientation::Vertical, 12);
    sin.set_valign(Align::Center);
    sin.set_halign(Align::Center);
    let sin_icono = icono("edit-find-symbolic", 72);
    sin_icono.add_css_class("empty-icon");
    sin.append(&sin_icono);
    let sin_titulo = Label::new(Some("Sin coincidencias"));
    sin_titulo.add_css_class("empty-titulo");
    sin.append(&sin_titulo);
    let sin_sub = Label::new(Some("Prueba con otro texto o cambia el filtro de tipo"));
    sin_sub.add_css_class("empty-sub");
    sin.append(&sin_sub);
    let sin_btn = boton_con_icono(
        "btn-ghost",
        "edit-clear-symbolic",
        "Limpiar búsqueda y filtros",
    );
    sin_btn.set_margin_top(8);
    sin.append(&sin_btn);
    paginas_biblio.add_titled(&sin, Some("sin"), "Sin resultados");

    pagina_bib.append(&paginas_biblio);

    // ── Página: ajustes ───────────────────────────────────────
    let pagina_aj = ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    pagina_aj.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);

    // ── Página: perfiles ──────────────────────────────────────
    let pagina_perf = ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    pagina_perf.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);
    let lista_perfiles = ListBox::new();
    lista_perfiles.add_css_class("settings-card");
    lista_perfiles.set_selection_mode(SelectionMode::None);

    // ── Aviso emergente ───────────────────────────────────────
    let toast = Revealer::new();
    toast.set_transition_type(RevealerTransitionType::SlideUp);
    toast.set_transition_duration(220);
    toast.set_valign(Align::End);
    toast.set_halign(Align::Center);
    toast.set_margin_bottom(26);
    toast.set_reveal_child(false);
    let toast_caja = GtkBox::new(Orientation::Horizontal, 10);
    toast_caja.add_css_class("toast");
    let toast_icon = icono("emblem-ok-symbolic", 16);
    let toast_label = Label::new(Some(""));
    toast_caja.append(&toast_icon);
    toast_caja.append(&toast_label);
    toast.set_child(Some(&toast_caja));

    // ── Diálogo de confirmación (capa superpuesta) ────────────
    let lightbox = Overlay::new();
    lightbox.add_css_class("lightbox");
    lightbox.set_visible(false);
    lightbox.set_hexpand(true);
    lightbox.set_vexpand(true);

    let fondo = GtkBox::new(Orientation::Vertical, 0);
    fondo.set_halign(Align::Fill);
    fondo.set_valign(Align::Fill);
    lightbox.set_child(Some(&fondo));

    let dialogo = GtkBox::new(Orientation::Vertical, 14);
    dialogo.add_css_class("dialog-card");
    dialogo.set_halign(Align::Center);
    dialogo.set_valign(Align::Center);
    dialogo.set_size_request(400, -1);
    let lightbox_titulo = Label::new(Some("¿Eliminar este fondo?"));
    lightbox_titulo.add_css_class("dialog-titulo");
    lightbox_titulo.set_halign(Align::Start);
    dialogo.append(&lightbox_titulo);
    let lightbox_sub = Label::new(Some(""));
    lightbox_sub.add_css_class("dialog-sub");
    lightbox_sub.set_halign(Align::Start);
    lightbox_sub.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    lightbox_sub.set_max_width_chars(44);
    dialogo.append(&lightbox_sub);

    let fila_btns = GtkBox::new(Orientation::Horizontal, 10);
    fila_btns.set_halign(Align::End);
    fila_btns.set_margin_top(6);
    let btn_cancelar = boton_con_icono("btn-ghost", "window-close-symbolic", "Cancelar");
    let btn_eliminar = boton_con_icono("btn-danger", "user-trash-symbolic", "Eliminar");
    fila_btns.append(&btn_cancelar);
    fila_btns.append(&btn_eliminar);
    dialogo.append(&fila_btns);
    lightbox.add_overlay(&dialogo);

    // ── Estructura principal ──────────────────────────────────
    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::SlideLeftRight);
    stack.set_transition_duration(220);
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.add_titled(&pagina_bib, Some("biblioteca"), "Biblioteca");

    let cuerpo = GtkBox::new(Orientation::Horizontal, 0);
    cuerpo.set_hexpand(true);
    cuerpo.set_vexpand(true);
    cuerpo.append(&sidebar);
    cuerpo.append(&stack);

    let raiz = Overlay::new();
    raiz.set_child(Some(&cuerpo));
    raiz.add_overlay(&toast);
    raiz.add_overlay(&lightbox);
    window.set_child(Some(&raiz));

    // ── Estado compartido ─────────────────────────────────────
    let lbl_info_hotkey = Label::new(Some("—"));
    let lbl_info_hotkey_siguiente = Label::new(Some("—"));
    let lbl_info_servidor = Label::new(Some("Apagado"));
    let ui = Rc::new(AppUi {
        config: RefCell::new(config),
        dir,
        flow: flow.clone(),
        paginas_biblio: paginas_biblio.clone(),
        items: RefCell::new(Vec::new()),
        contador: contador.clone(),
        hero_pic: hero_pic.clone(),
        hero_titulo: hero_titulo.clone(),
        hero_sub: hero_sub.clone(),
        hero_chips: hero_chips.clone(),
        btn_pausa: btn_pausa.clone(),
        btn_quitar: btn_quitar.clone(),
        hero_monitor: hero_monitor.clone(),
        monitor_dd: monitor_dd.clone(),
        monitor_sel: RefCell::new(Vec::new()),
        toast: toast.clone(),
        toast_icon: toast_icon.clone(),
        toast_label: toast_label.clone(),
        toast_gen: Cell::new(0),
        filtro: Cell::new(Filtro::Todo),
        query: RefCell::new(String::new()),
        search: search.clone(),
        chip_todo: chip_todo.clone(),
        dot_daemon: dot.clone(),
        txt_daemon: txt.clone(),
        lbl_hotkey: lbl_hotkey.clone(),
        lbl_deps: lbl_deps.clone(),
        lbl_info_hotkey,
        lbl_info_hotkey_siguiente,
        lbl_info_servidor,
        btn_iniciar: btn_iniciar.clone(),
        cuenta_lateral: cuenta.clone(),
        lightbox: lightbox.clone(),
        lightbox_titulo: lightbox_titulo.clone(),
        lightbox_sub: lightbox_sub.clone(),
        pendiente: RefCell::new(None),
        lista_perfiles: lista_perfiles.clone(),
    });

    // Ajustes (necesita el estado ya construido)
    pagina_aj.set_child(Some(&construir_ajustes(&ui)));
    stack.add_titled(&pagina_aj, Some("ajustes"), "Ajustes");

    // Perfiles (necesita el estado ya construido)
    pagina_perf.set_child(Some(&construir_perfiles(&ui)));
    stack.add_titled(&pagina_perf, Some("perfiles"), "Perfiles");

    // Clic en el fondo del diálogo de confirmación: cancelar
    {
        let ui2 = ui.clone();
        let cancel_click = GestureClick::new();
        cancel_click.connect_released(move |_, _, _, _| ui2.cerrar_lightbox());
        fondo.add_controller(cancel_click);
    }

    // ── Señales ───────────────────────────────────────────────
    {
        let ui2 = ui.clone();
        let stack2 = stack.clone();
        nav_biblio.connect_toggled(move |b| {
            if b.is_active() {
                stack2.set_visible_child_name("biblioteca");
                ui2.recargar();
            }
        });
        let stack3 = stack.clone();
        nav_ajustes.connect_toggled(move |b| {
            if b.is_active() {
                stack3.set_visible_child_name("ajustes");
            }
        });
        let stack4 = stack.clone();
        nav_perfiles.connect_toggled(move |b| {
            if b.is_active() {
                stack4.set_visible_child_name("perfiles");
            }
        });
    }

    {
        let ui2 = ui.clone();
        search.connect_search_changed(move |e| {
            *ui2.query.borrow_mut() = e.text().to_string();
            ui2.aplicar_visibilidad();
        });
    }

    {
        let ui2 = ui.clone();
        sin_btn.connect_clicked(move |_| ui2.restablecer_busqueda());
    }

    for (boton, filtro) in [
        (&chip_todo, Filtro::Todo),
        (&chip_video, Filtro::Video),
        (&chip_imagen, Filtro::Imagen),
        (&chip_favoritos, Filtro::Favoritos),
    ] {
        let ui2 = ui.clone();
        boton.connect_toggled(move |b| {
            if b.is_active() {
                ui2.filtro.set(filtro);
                ui2.aplicar_visibilidad();
            }
        });
    }

    // Orden de la biblioteca
    {
        let ui2 = ui.clone();
        orden_dd.connect_selected_notify(move |d| {
            let orden = match d.selected() {
                1 => Orden::NombreZa,
                2 => Orden::Fecha,
                3 => Orden::Tamano,
                4 => Orden::Manual,
                _ => Orden::NombreAz,
            };
            {
                let mut cfg = ui2.config.borrow_mut();
                // Al pasar a manual por primera vez, sembrar con el orden visible
                if orden == Orden::Manual && cfg.orden_manual.is_empty() {
                    cfg.orden_manual = ui2
                        .items
                        .borrow()
                        .iter()
                        .map(|i| i.path.to_string_lossy().to_string())
                        .collect();
                }
                cfg.orden_biblioteca = orden.id().to_string();
                cfg.guardar();
            }
            enviar_guardar_config(&ui2.config);
            ui2.recargar();
        });
    }

    // Selector de monitor: al cambiar, reflejar en el destacado
    {
        let ui2 = ui.clone();
        monitor_dd.connect_selected_notify(move |_| ui2.recargar());
    }

    {
        let ui2 = ui.clone();
        let win = window.clone();
        btn_add.connect_clicked(move |_| ui2.importar_dialogo(&win));
    }
    {
        let ui2 = ui.clone();
        let win = window.clone();
        vacio_btn.connect_clicked(move |_| ui2.importar_dialogo(&win));
    }
    {
        let ui2 = ui.clone();
        btn_aleatorio.connect_clicked(move |_| ui2.fondo_aleatorio());
    }
    {
        let ui2 = ui.clone();
        btn_pausa.connect_clicked(move |_| ui2.alternar_pausa_manual());
    }
    {
        let ui2 = ui.clone();
        btn_quitar.connect_clicked(move |_| ui2.quitar_fondo());
    }
    {
        let ui2 = ui.clone();
        btn_iniciar.connect_clicked(move |_| iniciar_daemon(&ui2));
    }
    {
        let ui2 = ui.clone();
        btn_carpeta.connect_clicked(move |_| abrir_carpeta(&ui2.dir));
    }
    {
        let ui2 = ui.clone();
        btn_cancelar.connect_clicked(move |_| ui2.cerrar_lightbox());
    }
    {
        let ui2 = ui.clone();
        btn_eliminar.connect_clicked(move |_| ui2.eliminar_pendiente());
    }

    // Arrastrar y soltar archivos sobre la biblioteca
    {
        let destino = DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        pagina_bib.add_controller(destino.clone());
        let zona = pagina_bib.clone();
        destino.connect_enter(move |_, _, _| {
            zona.add_css_class("drop-zone");
            gdk::DragAction::COPY
        });
        let zona2 = pagina_bib.clone();
        destino.connect_leave(move |_| {
            zona2.remove_css_class("drop-zone");
        });
        let ui2 = ui.clone();
        destino.connect_drop(move |_, valor, _, _| {
            if let Ok(lista) = valor.get::<gdk::FileList>() {
                let archivos = lista.files();
                let r = ui2.importar_archivos(&archivos);
                match (r.añadidos, r.duplicados) {
                    (0, 0) => return false,
                    (n, 0) => ui2.toast(
                        &format!("{} fondos añadidos", n),
                        "folder-download-symbolic",
                    ),
                    (0, d) => ui2.toast(
                        &format!("{} duplicados omitidos", d),
                        "dialog-warning-symbolic",
                    ),
                    (n, d) => ui2.toast(
                        &format!("{} añadidos, {} duplicados omitidos", n, d),
                        "folder-download-symbolic",
                    ),
                }
                return true;
            }
            false
        });
    }

    // Estado del servidor al arrancar y refresco periódico
    ui.refrescar_daemon();
    {
        let ui2 = ui.clone();
        glib::timeout_add_local(Duration::from_secs(3), move || {
            ui2.refrescar_daemon();
            glib::ControlFlow::Continue
        });
    }

    ui.recargar();
    window.present();
}
