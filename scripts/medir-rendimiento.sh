#!/bin/bash
# Mide el consumo real del fondo animado de Azura:
# CPU por /proc (delta de ticks) y RAM por ps. Requiere el daemon en marcha.
#
# Uso: scripts/medir-rendimiento.sh [segundos] [ruta-del-fondo]
set -euo pipefail
export LC_ALL=C # números con punto decimal en awk

SEGUNDOS="${1:-15}"
FONDO="${2:-}"

if [[ -n "${FONDO}" ]]; then
    wallpaper-animado set "${FONDO}" >/dev/null
    sleep 2
fi

pids() { pgrep -x mpv || true; }

wallpaper-animado status | grep -E "Fondo|Decodificación" || true

if [[ -z "$(pids)" ]]; then
    echo "Sin proceso mpv: el fondo actual es una imagen nativa de Cinnamon."
    echo "Consumo: 0 % de CPU y 0 MB de RAM en reposo."
    exit 0
fi

jiffies() { # suma de ticks de CPU (usuario + sistema) de los procesos mpv
    local total=0 campos pid
    for pid in $(pids); do
        read -ra campos < "/proc/${pid}/stat" || continue
        total=$((total + campos[13] + campos[14]))
    done
    echo "$total"
}

anterior=$(jiffies)
anterior_ms=$(date +%s%3N)
muestras=()
echo "Muestreando ${SEGUNDOS} s ($(pids | wc -l) proceso(s) mpv)…"
for _ in $(seq "${SEGUNDOS}"); do
    sleep 1
    ahora=$(jiffies)
    ahora_ms=$(date +%s%3N)
    ms=$((ahora_ms - anterior_ms))
    muestras+=("$(awk "BEGIN {printf \"%.1f\", (($ahora - $anterior) * 100 * 1000) / ($ms * $(getconf CLK_TCK))}")")
    anterior=$ahora
    anterior_ms=$ahora_ms
done

printf "%s\n" "${muestras[@]}" | awk '
    { s += $1; if ($1 > max || NR == 1) max = $1; n++ }
    END { printf "CPU media: %.1f %%   CPU máxima: %.1f %%   muestras: %d\n", s/n, max, n }'

mem=$(ps -o rss= -p "$(pids | paste -sd, -)" | awk '{s+=$1} END {print s+0}')
awk "BEGIN {printf \"RAM de mpv: %.0f MB\n\", ${mem}/1024}"
