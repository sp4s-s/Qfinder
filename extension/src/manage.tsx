import {
  List, ActionPanel, Action, Icon, Alert, confirmAlert,
  showToast, Toast, Color
} from "@raycast/api";
import { useState, useEffect, useCallback } from "react";
import { exec } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

const CONFIG_PATH = path.join(os.homedir(), ".qfinder", "config.json");
const DEFAULT_DIRS = [
  "~/Downloads", "~/Desktop", "~/Documents",
  "~/Movies", "~/Pictures", "~/.qfinder/notes"
];

interface Config {
  watch_dirs?: string[];
  [key: string]: unknown;
}

function readConfig(): string[] {
  try {
    const raw = fs.readFileSync(CONFIG_PATH, "utf-8");
    return (JSON.parse(raw) as Config).watch_dirs ?? DEFAULT_DIRS;
  } catch {
    return [...DEFAULT_DIRS];
  }
}

function writeConfig(dirs: string[]) {
  fs.mkdirSync(path.dirname(CONFIG_PATH), { recursive: true });
  let config: Config = {};
  try { config = JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8")) as Config; } catch { /* empty */ }
  config.watch_dirs = dirs;
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2));
}

function restartDaemon(): Promise<void> {
  return new Promise((resolve) => {
    const uid = process.getuid?.() ?? 501;
    const plist = `${os.homedir()}/Library/LaunchAgents/com.raycast.qfinder.daemon.plist`;
    exec(
      `launchctl bootout gui/${uid} "${plist}" 2>/dev/null; sleep 1; rm -f ~/.qfinder/socket.sock; launchctl bootstrap gui/${uid} "${plist}"`,
      () => resolve()
    );
  });
}

export default function Manage() {
  const [dirs, setDirs] = useState<string[]>([]);
  const [dirty, setDirty] = useState(false);

  useEffect(() => { setDirs(readConfig()); }, []);

  const remove = useCallback((dir: string) => {
    setDirs(d => d.filter(x => x !== dir));
    setDirty(true);
  }, []);

  const apply = useCallback(async () => {
    writeConfig(dirs);
    const toast = await showToast({ style: Toast.Style.Animated, title: "Restarting daemon…" });
    await restartDaemon();
    setDirty(false);
    toast.style = Toast.Style.Success;
    toast.title = "Done — new folders will be indexed shortly";
  }, [dirs]);

  const reset = useCallback(async () => {
    await confirmAlert({
      title: "Reset to defaults?",
      message: "All custom folders will be removed.",
      primaryAction: { title: "Reset", style: Alert.ActionStyle.Destructive },
    });
    setDirs([...DEFAULT_DIRS]);
    setDirty(true);
  }, []);

  return (
    <List navigationTitle="Manage Qfinder">
      {dirs.map((dir) => (
        <List.Item
          key={dir}
          icon={{ source: Icon.Folder, tintColor: Color.Blue }}
          title={dir}
          accessories={[{ icon: { source: Icon.Circle, tintColor: Color.Green }, tooltip: "Watched" }]}
          actions={
            <ActionPanel>
              <Action
                title="Remove Folder"
                icon={Icon.Trash}
                style={Action.Style.Destructive}
                onAction={() => remove(dir)}
              />
              {dirty && (
                <Action
                  title="Apply & Restart Daemon"
                  icon={Icon.ArrowClockwise}
                  shortcut={{ modifiers: ["cmd"], key: "s" }}
                  onAction={apply}
                />
              )}
              <Action
                title="Reset to Defaults"
                icon={Icon.RotateAntiClockwise}
                onAction={reset}
              />
              <Action.Open
                title="Open in Finder"
                target={dir.replace("~", os.homedir())}
              />
            </ActionPanel>
          }
        />
      ))}
      {dirty && (
        <List.Item
          icon={{ source: Icon.Warning, tintColor: Color.Yellow }}
          title="Unsaved changes — press Cmd+S on any item to apply"
          actions={
            <ActionPanel>
              <Action title="Apply & Restart Daemon" icon={Icon.ArrowClockwise} onAction={apply} />
            </ActionPanel>
          }
        />
      )}
    </List>
  );
}
