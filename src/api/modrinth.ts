import { httpJson } from "../net/http";

/**
 * Modrinth API v2. Plugins, mods y modpacks (.mrpack) sin API key.
 * CurseForge: manifest de mods se resuelve SOLO con key de API (opcional).
 */

const MR = "https://api.modrinth.com/v2";

export interface ModrinthFile {
  url: string;
  filename: string;
  hashes?: { sha1?: string; sha512?: string };
  primary?: boolean;
}

export interface ModrinthVersion {
  id: string;
  version_number: string;
  game_versions: string[];
  loaders: string[];
  files: ModrinthFile[];
}

export async function latestVersion(
  slug: string,
  opts: { gameVersions?: string[]; loaders?: string[] } = {}
): Promise<ModrinthVersion> {
  const params = new URLSearchParams();
  if (opts.gameVersions?.length)
    params.set("game_versions", JSON.stringify(opts.gameVersions));
  if (opts.loaders?.length) params.set("loaders", JSON.stringify(opts.loaders));
  const list = (await httpJson(
    `${MR}/project/${encodeURIComponent(slug)}/version?${params}`
  )) as ModrinthVersion[];
  if (!Array.isArray(list) || list.length === 0)
    throw new Error(`Modrinth: sin versión de ${slug} para esos filtros`);
  return list[0];
}

export function primaryFile(v: ModrinthVersion): ModrinthFile {
  return v.files.find((f) => f.primary) ?? v.files[0];
}

// --- modpacks .mrpack ---

export interface MrpackEntry {
  path: string;
  env?: { client: string; server: string };
  downloads: string[];
  hashes: { sha1?: string; sha512?: string };
  fileSize: number;
}

export interface MrpackIndex {
  formatVersion: number;
  game: string;
  versionId: string;
  name: string;
  files: MrpackEntry[];
  dependencies: Record<string, string>;
}

export function parseMrpackIndex(indexJson: string): MrpackIndex {
  const idx = JSON.parse(indexJson) as MrpackIndex;
  if (idx.formatVersion !== 1) throw new Error(`Formato .mrpack no soportado: ${idx.formatVersion}`);
  return idx;
}

export function mrpackServerFiles(idx: MrpackIndex): MrpackEntry[] {
  return idx.files.filter((f) => f.env?.server !== "unsupported");
}

/** URL del .mrpack a partir de una URL de página de Modrinth (o directo .mrpack) */
export async function modrinthModpackFile(url: string): Promise<{ file: ModrinthFile; version: ModrinthVersion }> {
  const m = /modrinth\.com\/(?:modpack|project)\/([\w-]+)/.exec(url);
  const slug = m ? m[1] : url;
  const version = await latestVersion(slug, {});
  return { file: primaryFile(version), version };
}

// --- CurseForge (manifest del zip exportado del launcher) ---

export interface CfManifest {
  minecraft: {
    version: string;
    modLoaders: Array<{ id: string; primary: boolean }>;
  };
  manifestType: string;
  manifestVersion: number;
  name: string;
  files: Array<{ projectID: number; fileID: number; required: boolean }>;
  overrides: string;
}

export function parseCfManifest(json: string): CfManifest {
  const m = JSON.parse(json) as CfManifest;
  if (m.manifestType !== "minecraftModpack")
    throw new Error(`No es un manifest de modpack CurseForge: ${m.manifestType}`);
  return m;
}

export function cfLoaderKind(manifest: CfManifest): { engine: "forge" | "neoforge"; build: string } | null {
  const id = manifest.minecraft.modLoaders.find((l) => l.primary)?.id ?? "";
  if (id.startsWith("forge-")) return { engine: "forge", build: id.slice("forge-".length) };
  if (id.startsWith("neoforge-")) return { engine: "neoforge", build: id.slice("neoforge-".length) };
  return null;
}

/**
 * Descarga un fichero de modpack CurseForge. Requiere key de la API de CF
 * (env OPENBLOCK_CF_KEY); sin key, CurseForge no permite descargas directas.
 */
export async function curseforgeFileUrl(projectID: number, fileID: number): Promise<string> {
  const key = process.env.OPENBLOCK_CF_KEY;
  if (!key)
    throw new Error(
      `CurseForge necesita API key (env OPENBLOCK_CF_KEY) para bajar project ${projectID} file ${fileID}`
    );
  const res = await fetch(`https://api.curseforge.com/v1/mods/${projectID}/files/${fileID}/download-url`, {
    headers: { "x-api-key": key, Accept: "application/json" },
  });
  if (!res.ok) throw new Error(`CF API ${res.status} para ${projectID}/${fileID}`);
  const data = (await res.json()) as { data: string };
  return data.data;
}
