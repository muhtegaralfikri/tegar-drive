#!/usr/bin/env bash
set -euo pipefail

APP_DIR=/mnt/hdd/.apps/tegar-drive
ENV_FILE=/etc/tegar-drive.env
SERVICE=/etc/systemd/system/tegar-drive.service
URL=https://github.com/muhtegaralfikri/tegar-drive/releases/latest/download/tegar-drive-linux-arm64.tar.gz
TMP=$(mktemp -d)

cleanup() {
  rm -rf "$TMP"
}
trap cleanup EXIT

mkdir -p "$APP_DIR" /mnt/hdd/drive
curl -fsSL "$URL" -o "$TMP/tegar-drive.tar.gz"
tar -xzf "$TMP/tegar-drive.tar.gz" -C "$TMP"
install -m 755 "$TMP/tegar-drive" "$APP_DIR/tegar-drive"

if [ ! -f "$ENV_FILE" ]; then
  umask 077
  {
    echo "DRIVE_ROOT=/mnt/hdd/drive"
    echo "DRIVE_ADDR=0.0.0.0:8084"
    echo "DRIVE_USER=${DRIVE_USER:-tegar}"
    echo "DRIVE_PASSWORD=${DRIVE_PASSWORD:-change-me}"
  } > "$ENV_FILE"
fi

cat > "$SERVICE" <<'EOF'
[Unit]
Description=Tegar Drive
Requires=mnt-hdd.mount
After=network-online.target mnt-hdd.mount
Wants=network-online.target

[Service]
EnvironmentFile=/etc/tegar-drive.env
ExecStart=/mnt/hdd/.apps/tegar-drive/tegar-drive
Restart=always
RestartSec=3
User=root

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now tegar-drive
systemctl restart tegar-drive
systemctl --no-pager --full status tegar-drive | sed -n '1,12p'
