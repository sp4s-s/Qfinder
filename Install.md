# qfinder — install guide

## prereqs

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh && source $HOME/.cargo/env
brew install node bun
npm install -g @raycast/api
```

## install

```bash
curl -fsSL https://raw.githubusercontent.com/sp4s-s/Qfinder/main/install.sh | bash
```

that's it. the daemon auto-starts at login via launchd.

## verify it's working

```bash
# check daemon is alive
launchctl list | grep qfinder

# check files are indexed
sqlite3 ~/.qfinder/db.sqlite "SELECT count(*), type FROM items GROUP BY type;"

# test search directly
python3 -c "
import socket, struct, json, os
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(os.path.expanduser('~/.qfinder/socket.sock'))
req = json.dumps({'command':'search','query':'test'}).encode()
s.sendall(struct.pack('>I', len(req)) + req)
n = struct.unpack('>I', s.recv(4))[0]
d = b''
while len(d)<n: d += s.recv(n-len(d))
print(json.loads(d)[:3])
s.close()
"
```

## use in raycast

open Raycast → type **Search qfinder** → start typing anything

- **Enter** — opens file
- **Cmd+1–9** — reveal that result in Finder
- **Cmd+Shift+C** — copy path

## dev / debug mode

```bash
cd extension && npm run dev
```

in dev mode every result shows a **match %** next to the type label.

## re-deploy after code changes

```bash
./install.sh
```

## uninstall

```bash
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.raycast.qfinder.daemon.plist
rm -rf ~/.qfinder ~/Library/LaunchAgents/com.raycast.qfinder.daemon.plist
```
