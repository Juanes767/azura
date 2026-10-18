//! CLI de control del servidor de fondos animados.
//!
//! Habla con el daemon por el socket IPC (una línea JSON) y traduce la
//! respuesta a salida legible. Pensada para scripts y teclas rápidas.

use serde_json::json;
use std::process::ExitCode;
use wallpaper_core as core;

const AYUDA: &str = "\
Azura — control del servidor

Uso: wallpaper-animado <comando> [opciones]

Comandos:
  status                        Muestra el estado del servidor
  set <archivo> [--monitor M]   Aplica un fondo (global o a un monitor)
  next | siguiente              Pasa al siguiente fondo
  random | aleatorio            Aplica un fondo al azar
  pause | resume                Pausa o reanuda el fondo
  toggle-pause                  Alterna la pausa
  clear                         Quita el fondo de todas las pantallas
  open | menu                   Abre el menú gráfico
  quit                          Detiene el servidor
  help | version

Ejemplos:
  wallpaper-animado set ~/Vídeos/fondo.mp4
  wallpaper-animado set fondo.mp4 --monitor DP-1
  wallpaper-animado next
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");

    match cmd {
        "help" | "-h" | "--help" => {
            print!("{}", AYUDA);
            ExitCode::SUCCESS
        }
        "version" | "-V" | "--version" => {
            println!("wallpaper-animado {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "status" => con_servidor(json!({ "cmd": "status" }), imprimir_estado),
        "set" => comando_set(&args[1..]),
        "next" | "siguiente" => simple(json!({ "cmd": "siguiente" }), "Fondo siguiente aplicado"),
        "random" | "aleatorio" => simple(json!({ "cmd": "aleatorio" }), "Fondo aleatorio aplicado"),
        "pause" => simple(json!({ "cmd": "pause" }), "Fondo en pausa"),
        "resume" => simple(json!({ "cmd": "resume" }), "Fondo reanudado"),
        "toggle-pause" | "toggle_pause" => {
            simple(json!({ "cmd": "toggle_pause" }), "Pausa alternada")
        }
        "clear" => simple(json!({ "cmd": "clear" }), "Fondo quitado"),
        "open" | "menu" => simple(json!({ "cmd": "show_menu" }), "Menú abierto"),
        "quit" | "exit" => simple(json!({ "cmd": "quit" }), "Servidor detenido"),
        otro => {
            eprintln!("Comando desconocido: {}", otro);
            eprintln!("Prueba con: wallpaper-animado help");
            ExitCode::from(1)
        }
    }
}

/// Envía un comando que solo responde `ok` y muestra la confirmación.
fn simple(cmd: serde_json::Value, exito: &str) -> ExitCode {
    con_servidor(cmd, |_| {
        println!("{}", exito);
        ExitCode::SUCCESS
    })
}

/// Aplica un fondo con validaciones locales antes de enviarlo al servidor.
fn comando_set(args: &[String]) -> ExitCode {
    let mut ruta: Option<&String> = None;
    let mut monitor: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--monitor" | "-m" => {
                monitor = args.get(i + 1).cloned();
                if monitor.is_none() {
                    eprintln!("Falta el nombre del monitor tras --monitor");
                    return ExitCode::from(1);
                }
                i += 2;
            }
            _ if ruta.is_none() => {
                ruta = Some(&args[i]);
                i += 1;
            }
            otro => {
                eprintln!("Argumento desconocido: {}", otro);
                return ExitCode::from(1);
            }
        }
    }

    let Some(ruta) = ruta else {
        eprintln!("Uso: wallpaper-animado set <archivo> [--monitor M]");
        return ExitCode::from(1);
    };

    let ruta = match std::fs::canonicalize(ruta) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("No se pudo abrir «{}»: {}", ruta, e);
            return ExitCode::from(1);
        }
    };
    if !ruta.is_file() {
        eprintln!("No es un archivo regular: {}", ruta.display());
        return ExitCode::from(1);
    }
    if !core::extension_ok(&ruta) {
        eprintln!("Formato no soportado (usa mp4, webm, mkv, mov, gif, png, jpg o jpeg)");
        return ExitCode::from(1);
    }

    let mut cmd = json!({ "cmd": "set_wallpaper", "path": ruta.to_string_lossy() });
    if let Some(m) = &monitor {
        cmd["monitor"] = json!(m);
    }

    let nombre = core::nombre_base(&ruta);
    let destino = monitor.map(|m| format!(" en «{}»", m)).unwrap_or_default();
    con_servidor(cmd, move |_| {
        println!("Aplicando «{}»{}", nombre, destino);
        ExitCode::SUCCESS
    })
}

/// Ejecuta el comando contra el daemon y delega la salida en `mostrar`.
fn con_servidor<F>(cmd: serde_json::Value, mostrar: F) -> ExitCode
where
    F: FnOnce(&serde_json::Value) -> ExitCode,
{
    match core::ipc(&cmd) {
        Ok(respuesta) => {
            if respuesta.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                mostrar(&respuesta)
            } else {
                let error = respuesta
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("comando rechazado");
                eprintln!("Error: {}", error);
                ExitCode::from(1)
            }
        }
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                || e.kind() == std::io::ErrorKind::ConnectionRefused =>
        {
            eprintln!("El servidor no está en ejecución (ábrelo desde el menú o con run.sh)");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("No se pudo hablar con el servidor: {}", e);
            ExitCode::from(1)
        }
    }
}

/// Imprime el estado del daemon en formato legible.
fn imprimir_estado(valor: &serde_json::Value) -> ExitCode {
    let Ok(estado) = serde_json::from_value::<core::EstadoDaemon>(valor.clone()) else {
        eprintln!("Respuesta de estado inválida");
        return ExitCode::from(1);
    };

    let fondo = estado
        .wallpaper
        .as_ref()
        .map(|p| {
            let ruta = std::path::Path::new(p);
            format!("{} ({})", core::nombre_base(ruta), p)
        })
        .unwrap_or_else(|| "ninguno".into());

    println!(
        "Servidor:          activo (v{}, {})",
        estado.version,
        duracion(estado.uptime_seg)
    );
    println!("Fondo:             {}", fondo);
    println!("Atajo menú:        {}", estado.hotkey);
    println!("Atajo siguiente:   {}", estado.hotkey_siguiente);
    println!("Pausa manual:      {}", si_no(estado.pausa_manual));
    println!("Pantalla bloqueada:{}", si_no(estado.pantalla_bloqueada));
    println!("Monitores:         {}", estado.monitores.join(", "));
    if !estado.fondos_por_monitor.is_empty() {
        for (monitor, ruta) in &estado.fondos_por_monitor {
            println!(
                "  {} → {}",
                monitor,
                core::nombre_base(std::path::Path::new(ruta))
            );
        }
    }
    println!(
        "Dependencias:      mpv {}, ffmpeg {}",
        if estado.mpv_disponible { "ok" } else { "falta" },
        if estado.ffmpeg_disponible {
            "ok"
        } else {
            "falta"
        }
    );
    ExitCode::SUCCESS
}

fn si_no(valor: bool) -> &'static str {
    if valor {
        "sí"
    } else {
        "no"
    }
}

/// Formatea segundos como «2 h 5 min», «3 min 12 s» o «45 s».
fn duracion(seg: u64) -> String {
    let (h, m, s) = (seg / 3600, (seg % 3600) / 60, seg % 60);
    if h > 0 {
        format!("{} h {} min", h, m)
    } else if m > 0 {
        format!("{} min {} s", m, s)
    } else {
        format!("{} s", s)
    }
}
