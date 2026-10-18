# Changelog

Todos los cambios importantes de este proyecto se documentan en este archivo.

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto se adhiere a [Versionado Semántico](https://semver.org/lang/es/).

## [Sin publicar]

### Añadido

- Paquete `.deb` (binarios, iconos, lanzador y unidad systemd de usuario)
  y publicación automática de releases al crear un tag `v*`.
- Diagnóstico real de decodificación: `wallpaper-animado status` y
  Ajustes → Reproducción muestran si mpv usa hardware o CPU.
- Las imágenes (PNG/JPG) se aplican como fondo nativo de Cinnamon sin
  proceso de vídeo: 0 % de CPU en reposo.
- `scripts/medir-rendimiento.sh` y [docs/rendimiento.md](docs/rendimiento.md)
  con mediciones reales (12.3 % CPU con VAAPI frente a 107.6 % por CPU).
- Sección del README comparando Azura con Hidamari.
- Colecciones con nombre: añade fondos desde la vista previa, filtra la
  biblioteca por colección y limita el cambio automático a una de ellas.
- Historial de recientes: los últimos 30 fondos aplicados, también como
  filtro de la biblioteca.
- Importar una carpeta completa de una vez (recursivo, sin duplicados).
- Limpieza automática al abrir: quita referencias a fondos inexistentes de
  favoritos, etiquetas, colecciones, historial y orden manual.

### Corregido

- `gsettings set` ya no usa `--` (gsettings lo rechaza): los temas de los
  perfiles y los selectores de «Aspecto del sistema» vuelven a aplicarse
  de verdad.
- El logo y los iconos del menú ya no dependen de la ruta del código
  fuente: funcionan instalados desde el paquete.

- Página de perfiles ampliada: sección **Aspecto del sistema** con selectores
  de tema de iconos (y color de carpetas), tema GTK, tema de ventanas, tema de
  Cinnamon y cursor; al elegir uno se aplica al instante.
- Los perfiles ahora guardan y restauran también el tema de iconos y el cursor,
  y los selectores se sincronizan al aplicar un perfil.
- Prueba del escaneo de temas instalados en el sistema.

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
