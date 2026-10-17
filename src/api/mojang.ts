import { httpJson } from "../net/http";

const MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

export interface McVersion {
  id: string;
  type: "release" | "snapshot" | "old_beta" | "old_alpha";
  url: string;
  releaseTime: string;
}

interface Manifest {
  latest: { release: string; snapshot: string };
  versions: McVersion[];
}

let manifestCache: Manifest | null = null;

export async function versionManifest(): Promise<Manifest> {
  if (!manifestCache) manifestCache = (await httpJson(MANIFEST)) as Manifest;
  return manifestCache;
}

export async function latestRelease(): Promise<string> {
  return (await versionManifest()).latest.release;
}

export async function listReleases(limit = 10): Promise<string[]> {
  const m = await versionManifest();
  return m.versions.filter((v) => v.type === "release").slice(0, limit).map((v) => v.id);
}

export interface ServerDownload {
  url: string;
  sha1: string;
}

export async function vanillaServerFor(mc: string): Promise<ServerDownload> {
  const m = await versionManifest();
  const v = m.versions.find((x) => x.id === mc);
  if (!v) throw new Error(`Versión de Minecraft desconocida: ${mc}`);
  const meta = (await httpJson(v.url)) as {
    downloads: { server: { url: string; sha1: string } };
  };
  if (!meta.downloads?.server) throw new Error(`MC ${mc} no tiene server.jar publicado`);
  return { url: meta.downloads.server.url, sha1: meta.downloads.server.sha1 };
}
