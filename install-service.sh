#!/bin/bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"

# ── Servicio systemd de usuario ───────────────────────────────
mkdir -p "$UNIT_DIR"
cat > "$UNIT_DIR/wallpaper-animado.service" << UNIT_EOF
[Unit]
Description=Fondos de pantalla animados
After=graphical-session.target

[Service]
Type=simple
WorkingDirectory=$DIR
ExecStart="$DIR/target/release/wallpaper-daemon"
Restart=on-failure
Environment=RUST_LOG=info
Environment=PATH=/usr/local/bin:/usr/bin:/bin

# Endurecimiento: sin privilegios extra, /tmp privado y sistema de solo lectura.
# La app solo escribe en $HOME (~/.config, ~/.cache) y en $XDG_RUNTIME_DIR.
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=full

[Install]
WantedBy=default.target
UNIT_EOF
systemctl --user daemon-reload
systemctl --user enable wallpaper-animado.service
systemctl --user start wallpaper-animado.service

# ── Lanzador de escritorio + icono ────────────────────────────
mkdir -p "$DATA_DIR/applications" "$DATA_DIR/icons/hicolor"
cp -r "$DIR/assets/icons/hicolor/." "$DATA_DIR/icons/hicolor/"
rm -f "$DATA_DIR/icons/hicolor/scalable/apps/wallpaper-animado.svg"
cat > "$DATA_DIR/applications/wallpaper-animado.desktop" << DESKTOP_EOF
[Desktop Entry]
Type=Application
Name=Azura
Comment=Gestiona y aplica fondos de pantalla animados
Exec="$DIR/target/release/wallpaper-menu"
Icon=wallpaper-animado
Terminal=false
Categories=Utility;Graphics;
Keywords=fondo;wallpaper;animado;pantalla;azura;
StartupWMClass=com.mint.wallpaper-animado
DESKTOP_EOF

# ── Comando de terminal ───────────────────────────────────────
mkdir -p "$BIN_DIR"
ln -sf "$DIR/target/release/wallpaper-animado" "$BIN_DIR/wallpaper-animado"

# Refrescar cachés (si las herramientas están disponibles)
update-desktop-database "$DATA_DIR/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "$DATA_DIR/icons/hicolor" 2>/dev/null || true

echo "✅ Servicio instalado desde $DIR"
echo "Ver estado:  systemctl --user status wallpaper-animado"
echo "Ver logs:    journalctl --user -u wallpaper-animado -f"
echo "Menú:        abre «Azura» en el lanzador de aplicaciones"
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "⚠️  $BIN_DIR no está en PATH: añádelo para usar el comando 'wallpaper-animado'"
fi
