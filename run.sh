#!/bin/bash
cd "$(dirname "$0")"
if [ ! -f target/release/wallpaper-daemon ]; then
    echo "🔨 Compilando..."
    cargo build --release
fi
echo "🚀 Iniciando daemon (Ctrl+C para salir)..."
RUST_LOG=info ./target/release/wallpaper-daemon
