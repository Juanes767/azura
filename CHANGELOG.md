# Changelog

Todos los cambios importantes de este proyecto se documentan en este archivo.

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto se adhiere a [Versionado Semántico](https://semver.org/lang/es/).

## [Sin publicar]

### Seguridad

- Los sockets IPC, de mpv y de la vista previa viven ahora en
  `$XDG_RUNTIME_DIR/wallpaper-animado/` con permisos `0700`, en lugar de
  rutas predecibles en `/tmp`.
- El daemon rechaza conexiones IPC de otros usuarios (verificación
  `SO_PEERCRED`) y limita el tamaño de los mensajes recibidos.
- Los perfiles validan nombres de archivo y extensiones antes de copiar nada,
  lo que evita rutas manipuladas (`../`) en un perfil alterado.
- `gsettings` recibe los valores tras `--` para que no se interpreten como
  opciones.
- La configuración y los perfiles se escriben de forma atómica (temporal +
  rename) para evitar archivos corruptos.
- El servicio systemd añade `NoNewPrivileges`, `PrivateTmp`, `ProtectSystem`
  y un `PATH` fijo.

### Cambiado

- `set_wallpaper` verifica en el daemon que la ruta sea un archivo regular con
  formato soportado, no solo que exista.
- Al salir, el daemon detiene los reproductores mpv en vez de dejarlos
  huérfanos.
- El error de `wallpaper-animado set` con una ruta inválida ahora indica el
  motivo real.

### Añadido

- Pruebas para el directorio de sockets, saneo de perfiles, escritura atómica
  y eliminación segura de sockets.
- Integración continua: formato, Clippy sin advertencias, pruebas, build de
  release y auditoría de dependencias.
- `SECURITY.md` con la política de reporte de vulnerabilidades.

## [0.1.0] - 2026-10-17

### Añadido

- Primera versión de Azura: biblioteca de fondos animados, miniaturas,
  metadatos, perfiles (en desarrollo), pausas inteligentes, multi-monitor,
  atajos globales, notificaciones, bandeja del sistema y línea de comandos.
