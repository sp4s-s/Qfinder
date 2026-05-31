import { List, ActionPanel, Action, Icon, Image, Keyboard, environment, showToast, Toast } from "@raycast/api";
import { useState, useEffect } from "react";
import { daemon } from "./daemon";
import { exec } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Manage from "./manage";

const DEBUG = environment.isDevelopment;
const CONFIG_PATH = path.join(os.homedir(), ".qfinder", "config.json");

interface ResultItem {
  id: number;
  type: "file" | "note" | "clipboard" | "image" | "video" | "pdf";
  title: string;
  path?: string;
  score: number;
  match_pct: number;
}

interface Config {
  clipboard_enabled?: boolean;
}

function openInFinder(p: string) {
  exec(`open -R "${p.replace(/"/g, '\\"')}"`);
}

function getConfig(): Config {
  try {
    return JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8")) as Config;
  } catch {
    return {};
  }
}

function restartDaemon() {
  const uid = process.getuid?.() ?? 501;
  const plist = `${os.homedir()}/Library/LaunchAgents/com.raycast.qfinder.daemon.plist`;
  exec(
    `launchctl bootout gui/${uid} "${plist}" 2>/dev/null; sleep 1; rm -f ~/.qfinder/socket.sock; launchctl bootstrap gui/${uid} "${plist}"`
  );
}

export default function Command() {
  const [searchText, setSearchText] = useState("");
  const [results, setResults] = useState<ResultItem[]>([]);
  const [clipEnabled, setClipEnabled] = useState(getConfig().clipboard_enabled !== false);

  useEffect(() => {
    if (searchText.trim().length === 0) {
      setResults([]);
      return;
    }
    daemon.search(searchText).then((r) => {
      if (Array.isArray(r)) setResults(r as ResultItem[]);
    });
  }, [searchText]);

  const toggleClipboard = async () => {
    const config = getConfig();
    const nextState = !(config.clipboard_enabled !== false);
    config.clipboard_enabled = nextState;
    fs.mkdirSync(path.dirname(CONFIG_PATH), { recursive: true });
    fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2));
    setClipEnabled(nextState);
    showToast({
      title: nextState ? "Clipboard Saving Enabled" : "Clipboard Saving Disabled",
      style: Toast.Style.Success,
    });
    restartDaemon();
  };

  const emptyClipboard = async () => {
    await daemon.clearClipboard();
    showToast({ title: "Clipboard Emptied", style: Toast.Style.Success });
    if (searchText.trim().length > 0) {
      daemon.search(searchText).then((r) => {
        if (Array.isArray(r)) setResults(r as ResultItem[]);
      });
    }
  };

  const icon = (item: ResultItem): Image.ImageLike => {
    if (item.type === "image" && item.path)
      return { source: item.path, mask: Image.Mask.RoundedRectangle };
    if (item.type === "video") return Icon.Video;
    if (item.type === "pdf") return Icon.Document;
    if (item.type === "note") return Icon.Pencil;
    if (item.type === "clipboard") return Icon.Clipboard;
    return Icon.Finder;
  };

  return (
    <List onSearchTextChange={setSearchText} searchBarPlaceholder="Search files, images, PDFs…">
      {results.map((r, idx) => {
        const shortcut: Keyboard.Shortcut | undefined =
          idx < 9
            ? { modifiers: ["cmd"] as Keyboard.KeyModifier[], key: String(idx + 1) as Keyboard.KeyEquivalent }
            : undefined;

        const accessories: List.Item.Accessory[] = [{ text: r.type.toUpperCase() }];
        if (DEBUG) accessories.unshift({ text: `${r.match_pct}%`, tooltip: `raw score: ${r.score.toFixed(2)}` });

        const clipContent = r.type === "clipboard" && r.path ? fs.readFileSync(r.path, "utf-8") : "";

        return (
          <List.Item
            key={r.id}
            icon={icon(r)}
            title={r.title}
            subtitle={r.path?.replace(`/${r.title}`, "") ?? ""}
            accessories={accessories}
            actions={
              <ActionPanel>
                <ActionPanel.Section>
                  {r.path && <Action.Open title="Open" target={r.path} />}
                  {r.path && (
                    <Action
                      title="Show in Finder"
                      icon={Icon.Finder}
                      shortcut={shortcut}
                      onAction={() => openInFinder(r.path!)}
                    />
                  )}
                  {r.path && <Action.CopyToClipboard title="Copy Path" content={r.path} />}
                  {r.type === "clipboard" && r.path && (
                    <Action.Paste title="Paste Clipboard Item" content={clipContent} />
                  )}
                </ActionPanel.Section>

                <ActionPanel.Section title="Qfinder Settings">
                  <Action.Push
                    title="Manage Folders"
                    icon={Icon.Folder}
                    target={<Manage />}
                    shortcut={{ modifiers: ["cmd"], key: "m" }}
                  />
                  <Action
                    title={clipEnabled ? "Disable Clipboard Saving" : "Enable Clipboard Saving"}
                    icon={clipEnabled ? Icon.Stop : Icon.Play}
                    onAction={toggleClipboard}
                  />
                  <Action
                    title="Empty Clipboard History"
                    icon={Icon.Trash}
                    shortcut={{ modifiers: ["cmd"], key: "d" }}
                    onAction={emptyClipboard}
                  />
                </ActionPanel.Section>
              </ActionPanel>
            }
          />
        );
      })}
    </List>
  );
}
