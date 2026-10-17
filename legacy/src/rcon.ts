import net from "node:net";

/**
 * Cliente RCON mínimo (protocolo de Valve). OpenBlock lo usa para automatizar
 * la pregeneración con Chunky y consultar spark sin tocar la consola.
 *
 * Packet: [len:i32le][id:i32le][type:i32le][payload ascii][0x00 0x00]
 */

export const SERVERDATA_AUTH = 3;
export const SERVERDATA_AUTH_RESPONSE = 2;
export const SERVERDATA_EXECCOMMAND = 2;
export const SERVERDATA_RESPONSE_VALUE = 0;

export function encodePacket(id: number, type: number, payload: string): Buffer {
  const body = Buffer.alloc(4 + 4 + Buffer.byteLength(payload) + 2);
  body.writeInt32LE(id, 0);
  body.writeInt32LE(type, 4);
  body.write(payload, 8, "ascii");
  // los 2 bytes 0x00 finales ya están a cero
  const full = Buffer.concat([Buffer.alloc(4), body]);
  full.writeInt32LE(body.length, 0);
  return full;
}

export interface DecodedPacket {
  id: number;
  type: number;
  payload: string;
}

export function decodePacket(buf: Buffer): DecodedPacket {
  const len = buf.readInt32LE(0);
  const id = buf.readInt32LE(4);
  const type = buf.readInt32LE(8);
  const payload = buf.subarray(12, 4 + len - 2).toString("ascii");
  return { id, type, payload };
}

export class RconClient {
  private socket?: net.Socket;
  private buffer = Buffer.alloc(0);
  private pending: Array<(p: DecodedPacket) => void> = [];
  private nextId = 1;

  constructor(
    private host: string,
    private port: number,
    private password: string
  ) {}

  async connect(timeoutMs = 8000): Promise<void> {
    await new Promise<void>((resolve, reject) => {
      this.socket = net.createConnection({ host: this.host, port: this.port }, resolve);
      this.socket.setTimeout(timeoutMs, () => reject(new Error("RCON timeout de conexión")));
      this.socket.on("error", reject);
      this.socket.on("data", (chunk) => this.onData(chunk));
    });
    const res = await this.send(SERVERDATA_AUTH, this.password);
    if (res.id === -1) throw new Error("RCON: contraseña rechazada");
  }

  private onData(chunk: Buffer): void {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 4) {
      const len = this.buffer.readInt32LE(0);
      if (this.buffer.length < 4 + len) break;
      const pkt = decodePacket(this.buffer.subarray(0, 4 + len));
      this.buffer = this.buffer.subarray(4 + len);
      const resolve = this.pending.shift();
      resolve?.(pkt);
    }
  }

  private send(type: number, payload: string): Promise<DecodedPacket> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("RCON: sin respuesta")), 15000);
      this.pending.push((p) => {
        clearTimeout(timer);
        resolve(p);
      });
      this.socket!.write(encodePacket(id, type, payload));
    });
  }

  async command(cmd: string): Promise<string> {
    const res = await this.send(SERVERDATA_EXECCOMMAND, cmd);
    return res.payload.trim();
  }

  end(): void {
    this.socket?.end();
    this.socket?.destroy();
  }
}

export async function withRcon<T>(
  host: string,
  port: number,
  password: string,
  fn: (rcon: RconClient) => Promise<T>
): Promise<T> {
  const rcon = new RconClient(host, port, password);
  await rcon.connect();
  try {
    return await fn(rcon);
  } finally {
    rcon.end();
  }
}
