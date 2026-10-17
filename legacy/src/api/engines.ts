import { httpJson } from "../net/http.js";
import { USER_AGENT } from "../version.js";

/**
 * Resolución de descargas por engine. Cada motor devuelve la URL del jar a
 * soltar en el directorio del server (o del installer, para forge/neoforge).
 *
 * Cada función de red tiene al lado su parte pura (`*From`): recibe la respuesta
 * **ya parseada** y devuelve exactamente lo mismo. Es lo único comparable —la
 * respuesta de una API ajena cambia sin avisar—, y es lo que el port a Rust
 * reproduce byte a byte contra `rust/crates/forge-api/tests/golden/engines.json`.
 */

export interface EngineDownload {
  url: string;
  sha256?: string;
  /** nombre final del jar en el directorio del server */
  filename: string;
  /** forge/neoforge: hay que ejecutar el installer antes del arranque */
  installer?: boolean;
  /** versión exacta resuelta (build de paper, loader de fabric...) */
  resolved: string;
}

export interface BuildInfo {
  id: string;
  channel?: string;
}

// --- familia PaperMC (fill v3 con fallback a v2) ---

const PAPERMC_PROJECTS = { paper: "paper", purpur: "purpur", folia: "folia" };

export interface PaperBuildRaw {
  id: number;
  channel?: string;
}

export interface PaperDownloadRaw {
  url: string;
  sha256?: string;
  name?: string;
}

export interface PaperBuildFullRaw {
  id: number;
  downloads: Record<string, PaperDownloadRaw>;
}

/** lista de builds disponibles de paper/folia para una versión de MC */
export async function paperBuilds(project: string, mc: string): Promise<BuildInfo[]> {
  try {
    return paperBuildsFrom(
      (await httpJson(
        `https://fill.papermc.io/v3/projects/${project}/versions/${encodeURIComponent(mc)}/builds`
      )) as PaperBuildRaw[]
    );
  } catch {
    return [];
  }
}

/** el mapeo de `paperBuilds`: nueva primero, y a la mínima se queda en vacío */
export function paperBuildsFrom(raw: PaperBuildRaw[]): BuildInfo[] {
  try {
    return raw.map((b) => ({ id: String(b.id), channel: b.channel })).sort((a, b) => Number(b.id) - Number(a.id));
  } catch {
    return [];
  }
}

async function papermcV3(project: string, mc: string, build?: string): Promise<EngineDownload | null> {
  try {
    const url = build
      ? `https://fill.papermc.io/v3/projects/${project}/versions/${encodeURIComponent(mc)}/builds/${encodeURIComponent(build)}`
      : `https://fill.papermc.io/v3/projects/${project}/versions/${encodeURIComponent(mc)}/builds?channel=STABLE`;
    return papermcV3From(await httpJson(url), project, mc);
  } catch {
    return null;
  }
}

/**
 * el mapeo de `papermcV3`. Ojo con el `reduce`: **no** es el máximo. El
 * acumulador vuelve a `builds[0]` en cuanto una build no supera a la anterior,
 * así que con los ids `[5, 7, 6]` el "última" que sale es la 5. Se reproduce tal
 * cual: el día que la API devuelva ese orden, el puerto tiene que mentir igual.
 */
export function papermcV3From(raw: unknown, project: string, mc: string): EngineDownload | null {
  try {
    // build exacta devuelve objeto único; lista devuelve array
    const builds = (Array.isArray(raw) ? raw : [raw]) as PaperBuildFullRaw[];
    if (builds.length === 0) return null;
    const latest = builds.reduce((a, b) => (b.id > a.id ? b : builds[0]), builds[0]);
    const dl = latest.downloads["server:default"] ?? Object.values(latest.downloads)[0];
    if (!dl) return null;
    return {
      url: dl.url,
      sha256: dl.sha256,
      filename: `${project}-${mc}-${latest.id}.jar`,
      resolved: `build ${latest.id}`,
    };
  } catch {
    return null;
  }
}

export interface PaperV2Raw {
  builds: Array<{ build: number; downloads: { application: { url: string; name: string; sha256?: string } } }>;
}

async function papermcV2(project: string, mc: string, build?: string): Promise<EngineDownload> {
  const data = (await httpJson(
    `https://api.papermc.io/v2/projects/${project}/versions/${encodeURIComponent(mc)}/builds`
  )) as PaperV2Raw;
  return papermcV2From(data, project, mc, build);
}

/** el mapeo de `papermcV2`: aquí sí se busca la build pedida, y si no está, lanza */
export function papermcV2From(data: PaperV2Raw, project: string, mc: string, build?: string): EngineDownload {
  const chosen = build ? data.builds.find((b) => String(b.build) === build) : data.builds[data.builds.length - 1];
  if (!chosen) throw new Error(`Sin build ${build} de ${project} para ${mc}`);
  return {
    url: chosen.downloads.application.url,
    sha256: chosen.downloads.application.sha256,
    filename: chosen.downloads.application.name,
    resolved: `build ${chosen.build}`,
  };
}

export interface PurpurBuildsRaw {
  builds?: { all?: string[] };
}

/** builds de Purpur para una versión de MC (nueva primero) */
export async function purpurBuilds(mc: string): Promise<BuildInfo[]> {
  return purpurBuildsFrom((await httpJson(`https://api.purpurmc.org/v2/purpur/${encodeURIComponent(mc)}`)) as PurpurBuildsRaw);
}

export function purpurBuildsFrom(data: PurpurBuildsRaw): BuildInfo[] {
  return [...(data.builds?.all ?? [])].reverse().map((id) => ({ id }));
}

export interface PurpurInfoRaw {
  build: { build: string; download: string };
  builds?: { latest: string; all?: string[] };
}

export function purpurDownloadFrom(info: PurpurInfoRaw, mc: string, build?: string): EngineDownload {
  const chosen = build ?? info.build.build;
  return {
    url: `https://api.purpurmc.org/v2/purpur/${encodeURIComponent(mc)}/${encodeURIComponent(chosen)}/download`,
    filename: `purpur-${mc}-${chosen}.jar`,
    resolved: `build ${chosen}`,
  };
}

export async function paperFamilyDownload(
  engine: "paper" | "purpur" | "folia",
  mc: string,
  build?: string
): Promise<EngineDownload> {
  if (engine === "purpur") {
    const info = (await httpJson(`https://api.purpurmc.org/v2/purpur/${encodeURIComponent(mc)}`)) as PurpurInfoRaw;
    return purpurDownloadFrom(info, mc, build);
  }
  const project = PAPERMC_PROJECTS[engine];
  const v3 = await papermcV3(project, mc, build);
  if (v3) return v3;
  return papermcV2(project, mc, build);
}

// --- Fabric / Quilt (meta APIs) ---

async function latestVersion(listUrl: string, field: string): Promise<string> {
  return latestVersionFrom((await httpJson(listUrl)) as Array<Record<string, unknown>>, field, listUrl);
}

/**
 * el `find` de las meta APIs. El filtro es `stable !== false`, así que una
 * entrada **sin** campo `stable` cuenta como estable: gana la primera de la
 * lista, no la primera estable de verdad.
 */
export function latestVersionFrom(list: Array<Record<string, unknown>>, field: string, listUrl: string): string {
  const first = list.find((e) => e.stable !== false) ?? list[0];
  if (!first) throw new Error(`Meta API vacía: ${listUrl}`);
  return String(first[field]);
}

export interface FabricLoaderRaw {
  loader?: { version: string };
  version?: string;
  stable?: boolean;
}

/** versiones de loader de Fabric disponibles para una versión de MC (nueva primero) */
export async function fabricLoaderVersions(mc: string): Promise<BuildInfo[]> {
  return fabricLoaderVersionsFrom(
    (await httpJson(`https://meta.fabricmc.net/v2/versions/loader/${encodeURIComponent(mc)}`)) as FabricLoaderRaw[]
  );
}

export function fabricLoaderVersionsFrom(list: FabricLoaderRaw[]): BuildInfo[] {
  return list.map((e) => ({ id: e.loader?.version ?? String(e.version), channel: e.stable === false ? "beta" : "stable" }));
}

export interface FabricInstallerRaw {
  version: string;
  stable?: boolean;
}

export async function fabricInstallerVersions(): Promise<BuildInfo[]> {
  return fabricInstallerVersionsFrom(
    (await httpJson("https://meta.fabricmc.net/v2/versions/installer")) as FabricInstallerRaw[]
  );
}

export function fabricInstallerVersionsFrom(list: FabricInstallerRaw[]): BuildInfo[] {
  return list.map((e) => ({ id: e.version, channel: e.stable === false ? "beta" : "stable" }));
}

export async function fabricDownload(mc: string, loaderBuild?: string, installerBuild?: string): Promise<EngineDownload> {
  const loader = loaderBuild ?? (await latestVersion("https://meta.fabricmc.net/v2/versions/loader", "version"));
  const installer = installerBuild ?? (await latestVersion("https://meta.fabricmc.net/v2/versions/installer", "version"));
  return fabricDownloadFrom(mc, loader, installer);
}

/** el loader y el installer van **sin** escapar, tal como los espera la meta API */
export function fabricDownloadFrom(mc: string, loader: string, installer: string): EngineDownload {
  return {
    url: `https://meta.fabricmc.net/v2/versions/loader/${encodeURIComponent(mc)}/${loader}/${installer}/server/jar`,
    filename: `fabric-server-${mc}.jar`,
    resolved: `loader ${loader}`,
  };
}

export interface QuiltLoaderRaw {
  version?: string;
  loader?: { version: string };
}

export async function quiltLoaderVersions(mc: string): Promise<BuildInfo[]> {
  return quiltLoaderVersionsFrom(
    (await httpJson(`https://meta.quiltmc.org/v3/versions/loader/${encodeURIComponent(mc)}`)) as QuiltLoaderRaw[]
  );
}

export function quiltLoaderVersionsFrom(list: QuiltLoaderRaw[]): BuildInfo[] {
  return list.map((e) => ({ id: e.loader?.version ?? String(e.version) }));
}

export interface QuiltInstallerRaw {
  version: string;
}

export async function quiltInstallerVersions(): Promise<BuildInfo[]> {
  return quiltInstallerVersionsFrom(
    (await httpJson("https://meta.quiltmc.org/v3/versions/installer")) as QuiltInstallerRaw[]
  );
}

export function quiltInstallerVersionsFrom(list: QuiltInstallerRaw[]): BuildInfo[] {
  return list.map((e) => ({ id: e.version }));
}

export async function quiltDownload(mc: string, loaderBuild?: string, installerBuild?: string): Promise<EngineDownload> {
  const loader = loaderBuild ?? (await latestVersion("https://meta.quiltmc.org/v3/versions/loader", "version"));
  const installer = installerBuild ?? (await latestVersion("https://meta.quiltmc.org/v3/versions/installer", "version"));
  return quiltDownloadFrom(mc, loader, installer);
}

export function quiltDownloadFrom(mc: string, loader: string, installer: string): EngineDownload {
  return {
    url: `https://meta.quiltmc.org/v3/versions/loader/${encodeURIComponent(mc)}/${loader}/${installer}/server/jar`,
    filename: `quilt-server-${mc}.jar`,
    resolved: `loader ${loader}`,
  };
}

// --- Forge ---

/** builds de Forge para una versión de MC (del maven-metadata; nueva primero) */
export async function forgeBuilds(mc: string): Promise<BuildInfo[]> {
  const xml = await fetch("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml", {
    headers: { "User-Agent": USER_AGENT },
  }).then((r) => {
    if (!r.ok) throw new Error(`Maven Forge HTTP ${r.status}`);
    return r.text();
  });
  return forgeBuildsFrom(xml, mc);
}

/**
 * el regex del maven-metadata. El prefijo va **sin escapar**: con `mc = "1.20.1"`
 * los puntos son comodines, así que también entra `1x20y1-47.2.0`. Y el `Set`
 * quita duplicados conservando la primera aparición, y **después** se invierte:
 * el resultado es el orden inverso de la primera aparición, no un orden de
 * versiones.
 */
export function forgeBuildsFrom(xml: string, mc: string): BuildInfo[] {
  const prefix = `${mc}-`;
  const versions = [...xml.matchAll(new RegExp(`<version>${prefix}([^<]+)</version>`, "g"))].map((m) => m[1]);
  return [...new Set(versions)].reverse().map((id) => ({ id }));
}

export interface ForgePromosRaw {
  promos: Record<string, string>;
}

export async function forgeDownload(mc: string, build?: string): Promise<EngineDownload> {
  let chosen = build;
  if (!chosen) {
    const promos = (await httpJson(
      "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json"
    )) as ForgePromosRaw;
    chosen = forgePickFrom(promos, mc) ?? "";
    if (!chosen) throw new Error(`Forge no soporta ${mc}`);
  }
  return forgeDownloadFrom(mc, chosen);
}

/** `recommended` y si no `latest`; vacío significa que no hay Forge para esa MC */
export function forgePickFrom(promos: ForgePromosRaw, mc: string): string | null {
  return promos.promos[`${mc}-recommended`] ?? promos.promos[`${mc}-latest`] ?? null;
}

export function forgeDownloadFrom(mc: string, chosen: string): EngineDownload {
  const base = `https://maven.minecraftforge.net/net/minecraftforge/forge/${mc}-${chosen}`;
  return {
    url: `${base}/forge-${mc}-${chosen}-installer.jar`,
    filename: `forge-installer-${mc}.jar`,
    installer: true,
    resolved: chosen,
  };
}

// --- NeoForge (1.20.2+; versión = prefijo de MC sin el "1.") ---

export interface NeoForgeRaw {
  versions: string[];
}

/** `1.20.2` → `20.2`: el número de NeoForge pierde el primer segmento de la MC */
export function neoforgePrefix(mc: string): string {
  return mc.split(".").slice(1).join(".");
}

export async function neoforgeBuilds(mc: string): Promise<BuildInfo[]> {
  const data = (await httpJson(
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge"
  )) as NeoForgeRaw;
  return neoforgeBuildsFrom(data, mc);
}

export function neoforgeBuildsFrom(data: NeoForgeRaw, mc: string): BuildInfo[] {
  return neoforgeVersionsFrom(data, mc).reverse().map((id) => ({ id }));
}

/** las versiones que empiezan por el prefijo de esa MC, en el orden de la API */
export function neoforgeVersionsFrom(data: NeoForgeRaw, mc: string): string[] {
  return data.versions.filter((v) => v.startsWith(`${neoforgePrefix(mc)}.`));
}

export async function neoforgeDownload(mc: string, build?: string): Promise<EngineDownload> {
  let chosen = build;
  if (!chosen) {
    const data = (await httpJson(
      "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge"
    )) as NeoForgeRaw;
    const candidates = neoforgeVersionsFrom(data, mc);
    if (candidates.length === 0) throw new Error(`NeoForge no soporta ${mc}`);
    chosen = candidates[candidates.length - 1];
  }
  return neoforgeDownloadFrom(mc, chosen);
}

export function neoforgeDownloadFrom(mc: string, chosen: string): EngineDownload {
  const base = `https://maven.neoforged.net/releases/net/neoforged/neoforge/${chosen}`;
  return {
    url: `${base}/neoforge-${chosen}-installer.jar`,
    filename: `neoforge-installer-${mc}.jar`,
    installer: true,
    resolved: chosen,
  };
}
