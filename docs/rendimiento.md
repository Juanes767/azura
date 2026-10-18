# Rendimiento

Mediciones reales del consumo de Azura, con metodología reproducible. Los
números son de un equipo concreto: no son una promesa universal.

## Equipo de medición

- CPU: Intel Pentium Gold 7505 (2 núcleos / 4 hilos)
- GPU: Intel UHD Graphics G4 (Tiger Lake-LP)
- Sesión: Linux Mint 22.3, Cinnamon, X11 (monitor eDP-1, 1080p)
- Fondo de prueba: vídeo 1080p (`emerald-leviathan…`, mpv con `--fps=30`)

## Metodología

`scripts/medir-rendimiento.sh` (requiere el daemon en marcha):

```bash
scripts/medir-rendimiento.sh 12                       # mide el fondo actual
scripts/medir-rendimiento.sh 12 ~/fondo.mp4           # aplica y mide
```

- **CPU**: delta de ticks de `/proc/<pid>/stat` (usuario + sistema) por
  segundo, sumando todos los procesos `mpv`.
- **RAM**: RSS de los procesos `mpv` (`ps -o rss`).
- **Decodificación**: valor real `hwdec-current` que reporta mpv (se muestra
  también en Ajustes → Reproducción y en `wallpaper-animado status`).

## Resultados (medidos el 18/10/2026)

| Escenario | CPU media | CPU máx | RAM | Procesos mpv |
| --- | --- | --- | --- | --- |
| Vídeo 1080p, decodificación VAAPI (hardware) | **12.3 %** | 13.3 % | 176 MB | 1 |
| Vídeo 1080p, decodificación por CPU forzada (`--hwdec=no`) | **107.6 %** | 122.6 % | 189 MB | 1 |
| Imagen estática (PNG) como fondo nativo de Cinnamon | **0 %** | 0 % | 0 MB | 0 |

Notas:

- La decodificación por hardware es ~8.7 veces más barata en CPU que la
  decodificación por software en este equipo; por eso el diagnóstico de
  decodificación en Ajustes avisa cuando mpv cae a software.
- Las imágenes se aplican como fondo nativo de Cinnamon
  (`org.cinnamon.desktop.background picture-uri`), sin ningún proceso de
  vídeo: consumo cero en reposo y la memoria de mpv (≈ 176 MB) desaparece.
- El límite de FPS configurado era 30 en todas las mediciones.

## Reproducir las mediciones

1. Aplica el vídeo de prueba: `wallpaper-animado set /ruta/fondo.mp4`.
2. Ejecuta `scripts/medir-rendimiento.sh 15` y comprueba que
   `Decodificación:` diga `hardware (vaapi)` o similar.
3. Para la referencia por software: `wallpaper-animado clear`, lanza mpv a
   mano con `--hwdec=no` sobre la ventana del daemon y repite el script.
4. Para la imagen: `wallpaper-animado set /ruta/imagen.png` y verifica que no
   quede ningún proceso `mpv` (`pgrep -x mpv`).
