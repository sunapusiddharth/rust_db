#!/bin/bash
# install-kvstore-service.sh

set -e

echo "Creating user and group..."
sudo useradd -r -s /usr/sbin/nologin -U kvstore || true

echo "Creating directories..."
sudo mkdir -p /etc/kvstore-plus-plus
sudo mkdir -p /var/lib/kvstore-plus-plus/data/wal
sudo mkdir -p /var/lib/kvstore-plus-plus/data/snapshots

echo "Setting permissions..."
sudo chown -R kvstore:kvstore /var/lib/kvstore-plus-plus
sudo chmod 700 /var/lib/kvstore-plus-plus

echo "Copying config..."
sudo cp config.toml /etc/kvstore-plus-plus/config.toml
sudo chown kvstore:kvstore /etc/kvstore-plus-plus/config.toml

echo "Copying binary..."
sudo cp target/release/kvstore-plus-plus /usr/local/bin/kvstore-plus-plus
sudo chown root:root /usr/local/bin/kvstore-plus-plus
sudo chmod 755 /usr/local/bin/kvstore-plus-plus

echo "Installing systemd service..."
sudo cp kvstore-plus-plus.service /etc/systemd/system/

echo "Reloading systemd..."
sudo systemctl daemon-reload

echo "Enabling service..."
sudo systemctl enable kvstore-plus-plus.service

echo "Starting service..."
sudo systemctl start kvstore-plus-plus.service

echo "Checking status..."
sudo systemctl status kvstore-plus-plus.service --no-pager

echo "Viewing logs..."
echo "Use: journalctl -u kvstore-plus-plus -f"