import * as net from "node:net";
import os from "node:os";
import path from "node:path";
import { showToast, Toast } from "@raycast/api";

const SOCKET_PATH = path.join(os.homedir(), ".qfinder", "socket.sock");

export class DaemonClient {
  private client: net.Socket | null = null;
  private pendingResolvers: ((value: unknown) => void)[] = [];
  private buffer: ReturnType<typeof Buffer.alloc> = Buffer.alloc(0);
  private retryDelay = 100;

  constructor() {
    this.connect();
  }

  private connect() {
    this.client = net.createConnection(SOCKET_PATH);

    this.client.on("data", (data) => {
      this.buffer = Buffer.concat([Uint8Array.from(this.buffer), Uint8Array.from(data)]);
      while (this.buffer.length >= 4) {
        const msgLen = this.buffer.readUInt32BE(0);
        if (this.buffer.length >= 4 + msgLen) {
          const payload = this.buffer.subarray(4, 4 + msgLen).toString("utf-8");
          this.buffer = this.buffer.subarray(4 + msgLen);
          const resolve = this.pendingResolvers.shift();
          if (resolve) resolve(JSON.parse(payload));
        } else {
          break;
        }
      }
    });

    this.client.on("error", () => {
      this.client = null;
      showToast({
        style: Toast.Style.Failure,
        title: "Daemon not running",
        message: "Please download and start the Qfinder app.",
      });
      this.retryDelay = Math.min(this.retryDelay * 2, 5000);
      setTimeout(() => this.connect(), this.retryDelay);
    });

    this.client.on("connect", () => {
      this.retryDelay = 100;
    });
  }

  private sendCommand(command: string, args: Record<string, unknown>): Promise<unknown> {
    return new Promise((resolve) => {
      if (!this.client) {
        resolve([]);
        return;
      }
      this.pendingResolvers.push(resolve);
      const json = JSON.stringify({ command, ...args });
      const payloadBytes = Buffer.from(json, "utf-8");
      const lenBuf = Buffer.alloc(4);
      lenBuf.writeUInt32BE(payloadBytes.length, 0);
      this.client.write(lenBuf.toString("binary"), "binary");
      this.client.write(json, "utf-8");
    });
  }

  public search(query: string, scope?: string): Promise<unknown> {
    return this.sendCommand("search", { query, scope });
  }

  public clearClipboard(): Promise<unknown> {
    return this.sendCommand("clear_clipboard", {});
  }
}

export const daemon = new DaemonClient();
