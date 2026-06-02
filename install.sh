#!/bin/bash
set -e

# Detect if we are in the cloned repository
if [ -d "daemon" ] && [ -d "extension" ]; then
    echo "=> Running installer from local repository..."
    REPO_DIR=$(pwd)
else
    echo "=> Downloading Qfinder repository..."
    TMP_DIR=$(mktemp -d)
    git clone https://github.com/sp4s-s/Qfinder.git "$TMP_DIR"
    cd "$TMP_DIR"
    REPO_DIR="$TMP_DIR"
fi

echo "=> Building daemon..."
cd "$REPO_DIR/daemon"
cargo build --release

echo "=> Installing daemon..."
mkdir -p ~/.qfinder/notes
cp target/release/qfinder-daemon ~/.qfinder/qfinder-daemon
chmod +x ~/.qfinder/qfinder-daemon

echo "=> Configuring launchd service..."
PLIST_PATH=~/Library/LaunchAgents/com.raycast.qfinder.daemon.plist

cat <<EOF > $PLIST_PATH
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.raycast.qfinder.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>$HOME/.qfinder/qfinder-daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF

launchctl unload $PLIST_PATH 2>/dev/null || true
launchctl load $PLIST_PATH

echo "=> Installing extension..."
rm -rf ~/.qfinder/extension
cp -r "$REPO_DIR/extension" ~/.qfinder/extension
cd ~/.qfinder/extension
bun install

echo "=> Building extension..."
bun run build

echo "=> Registering extension with Raycast..."
open -ga Raycast 2>/dev/null || true
sleep 2
./node_modules/.bin/ray develop -I >/tmp/qfinder-raycast-import.log 2>&1 &
RAY_DEV_PID=$!
sleep 6
kill "$RAY_DEV_PID" >/dev/null 2>&1 || true
wait "$RAY_DEV_PID" >/dev/null 2>&1 || true

echo "=> Finalizing production extension build..."
bun run build

echo ""
echo "✅ Qfinder (qfinder) installed successfully!"
echo "   The background daemon is running."
echo "   The Raycast extension has been registered."
echo "   Open Raycast and type: Search Qfinder"
echo ""
