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
bun run build

echo ""
echo "✅ Qfinder (qfinder) installed successfully!"
echo "   The background daemon is running."
echo ""
echo "   To use it in Raycast:"
echo "   1. Open Raycast Settings -> Extensions"
echo "   2. Click '+' -> Add Local Extension Directory"
echo "   3. Select the folder: ~/.qfinder/extension"
echo ""
