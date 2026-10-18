# Política de seguridad

## Versiones soportadas

| Versión | Soporte |
| ------- | ------- |
| 0.1.x   | Sí      |

## Cómo reportar una vulnerabilidad

Usa el formulario privado de **GitHub Security Advisories** del repositorio
(pestaña *Security* → *Report a vulnerability*). No abras un issue público
para vulnerabilidades.

Incluye, si es posible:

- Descripción del problema y su impacto.
- Pasos para reproducirlo.
- Versión de Azura (`wallpaper-animado version`) y distribución.
- Cualquier propuesta de mitigación.

## Qué esperar

- Confirmación de recepción en un plazo de 7 días.
- Una evaluación inicial y, si procede, un plan de corrección.
- Crédito en el aviso publicado, salvo que prefieras permanecer en el anonimato.

## Modelo de amenaza

Azura es una aplicación de escritorio local (Linux/X11) que se ejecuta con los
privilegios del usuario. Sus medidas principales:

- Los sockets (IPC del daemon, control de mpv y vista previa) viven en
  `$XDG_RUNTIME_DIR/wallpaper-animado/` con permisos `0700`, nunca en `/tmp`.
- El daemon solo acepta conexiones IPC del mismo usuario (verificación
  `SO_PEERCRED`) y limita el tamaño de los mensajes.
- Los perfiles validan nombres de archivo y extensiones antes de copiar nada.
- La configuración y los perfiles se escriben de forma atómica.
- El servicio systemd usa `NoNewPrivileges`, `PrivateTmp` y `ProtectSystem=full`.

Quedan fuera de alcance los ataques que ya requieren control de la sesión del
usuario (por ejemplo, editar la propia configuración o ejecutar código con su
mismo UID).

## Dependencias

La auditoría de dependencias (`cargo audit`) se ejecuta en la integración
continua en cada push y pull request.
