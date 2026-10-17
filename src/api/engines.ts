import { httpJson } from "../net/http";
import { USER_AGENT } from "../version";

/**
 * Resolución de descargas por engine. Cada motor devuelve la URL del jar a
 * soltar en el directorio del server (o del installer, para forge/neoforge).
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

// --- familia PaperMC (fill v3 con fallback a v2) ---

const PAPERMC_PROJECTS = { paper: "paper", purpur: "purpur", folia: "folia" };

export interface BuildInfo {
  id: string;
  channel?: string;
}

/** lista de builds disponibles de paper/folia para una versión de MC */
export async function paperBuilds(project: string, mc: string): Promise<BuildInfo[]> {
  try {
    const builds = (await httpJson(
      `https://fill.papermc.io/v3/projects/${project}/versions/${encodeURIComponent(mc)}/builds`
    )) as Array<{ id: number; channel?: string }>;
    return builds
      .map((b) => ({ id: String(b.id), channel: b.channel }))
      .sort((a, b) => Number(b.id) - Number(a.id));
  } catch {
    return [];
  }
}

async function papermcV3(project: string, mc: string, build?: string): Promise<EngineDownload | null> {
  try {
    const url = build
      ? `https://fill.papermc.io/v3/projects/${project}/versions/${encodeURIComponent(mc)}/builds/${encodeURIComponent(build)}`
      : `https://fill.papermc.io/v3/projects/${project}/versions/${encodeURIComponent(mc)}/builds?channel=STABLE`;
    const raw = await httpJson(url);
    // build exacta devuelve objeto único; lista devuelve array
    const builds = (Array.isArray(raw) ? raw : [raw]) as Array<{
      id: number;
      downloads: Record<string, { url: string; sha256?: string; name?: string }>;
    }>;
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

async function papermcV2(project: string, mc: string, build?: string): Promise<EngineDownload> {
  const data = (await httpJson(
    `https://api.papermc.io/v2/projects/${project}/versions/${encodeURIComponent(mc)}/builds`
  )) as {
    builds: Array<{ build: number; downloads: { application: { url: string; name: string; sha256?: string } } }>;
  };
  const chosen = build ? data.builds.find((b) => String(b.build) === build) : data.builds[data.builds.length - 1];
  if (!chosen) throw new Error(`Sin build ${build} de ${project} para ${mc}`);
  return {
    url: chosen.downloads.application.url,
    sha256: chosen.downloads.application.sha256,
    filename: chosen.downloads.application.name,
    resolved: `build ${chosen.build}`,
  };
}

/** builds de Purpur para una versión de MC (nueva primero) */
export async function purpurBuilds(mc: string): Promise<BuildInfo[]> {
  const data = (await httpJson(`https://api.purpurmc.org/v2/purpur/${encodeURIComponent(mc)}`)) as {
    builds?: { all?: string[] };
  };
  return [...(data.builds?.all ?? [])].reverse().map((id) => ({ id }));
}

export async function paperFamilyDownload(
  engine: "paper" | "purpur" | "folia",
  mc: string,
  build?: string
): Promise<EngineDownload> {
  if (engine === "purpur") {
    const info = (await httpJson(`https://api.purpurmc.org/v2/purpur/${encodeURIComponent(mc)}`)) as {
      build: { build: string; download: string };
      builds?: { latest: string; all?: string[] };
    };
    const chosen = build ?? info.build.build;
    return {
      url: `https://api.purpurmc.org/v2/purpur/${encodeURIComponent(mc)}/${encodeURIComponent(chosen)}/download`,
      filename: `purpur-${mc}-${chosen}.jar`,
      resolved: `build ${chosen}`,
    };
  }
  const project = PAPERMC_PROJECTS[engine];
  const v3 = await papermcV3(project, mc, build);
  if (v3) return v3;
  return papermcV2(project, mc, build);
}

// --- Fabric / Quilt (meta APIs) ---

async function latestVersion(listUrl: string, field: string): Promise<string> {
  const list = (await httpJson(listUrl)) as Array<Record<string, unknown>>;
  const first = list.find((e) => e.stable !== false) ?? list[0];
  if (!first) throw new Error(`Meta API vacía: ${listUrl}`);
  return String(first[field]);
}

/** versiones de loader de Fabric disponibles para una versión de MC (nueva primero) */
export async function fabricLoaderVersions(mc: string): Promise<BuildInfo[]> {
  const list = (await httpJson(
    `https://meta.fabricmc.net/v2/versions/loader/${encodeURIComponent(mc)}`
  )) as Array<{ loader?: { version: string }; version?: string; stable?: boolean }>;
  return list.map((e) => ({ id: e.loader?.version ?? String(e.version), channel: e.stable === false ? "beta" : "stable" }));
}

export async function fabricInstallerVersions(): Promise<BuildInfo[]> {
  const list = (await httpJson("https://meta.fabricmc.net/v2/versions/installer")) as Array<{
    version: string;
    stable?: boolean;
  }>;
  return list.map((e) => ({ id: e.version, channel: e.stable === false ? "beta" : "stable" }));
}

export async function fabricDownload(mc: string, loaderBuild?: string, installerBuild?: string): Promise<EngineDownload> {
  const loader = loaderBuild ?? (await latestVersion("https://meta.fabricmc.net/v2/versions/loader", "version"));
  const installer = installerBuild ?? (await latestVersion("https://meta.fabricmc.net/v2/versions/installer", "version"));
  return {
    url: `https://meta.fabricmc.net/v2/versions/loader/${encodeURIComponent(mc)}/${loader}/${installer}/server/jar`,
    filename: `fabric-server-${mc}.jar`,
    resolved: `loader ${loader}`,
  };
}

export async function quiltLoaderVersions(mc: string): Promise<BuildInfo[]> {
  const list = (await httpJson(
    `https://meta.quiltmc.org/v3/versions/loader/${encodeURIComponent(mc)}`
  )) as Array<{ version?: string; loader?: { version: string } }>;
  return list.map((e) => ({ id: e.loader?.version ?? String(e.version) }));
}

export async function quiltInstallerVersions(): Promise<BuildInfo[]> {
  const list = (await httpJson("https://meta.quiltmc.org/v3/versions/installer")) as Array<{ version: string }>;
  return list.map((e) => ({ id: e.version }));
}

export async function quiltDownload(mc: string, loaderBuild?: string, installerBuild?: string): Promise<EngineDownload> {
  const loader = loaderBuild ?? (await latestVersion("https://meta.quiltmc.org/v3/versions/loader", "version"));
  const installer = installerBuild ?? (await latestVersion("https://meta.quiltmc.org/v3/versions/installer", "version"));
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
  const prefix = `${mc}-`;
  const versions = [...xml.matchAll(new RegExp(`<version>${prefix}([^<]+)</version>`, "g"))].map((m) => m[1]);
  return [...new Set(versions)].reverse().map((id) => ({ id }));
}

export async function forgeDownload(mc: string, build?: string): Promise<EngineDownload> {
  let chosen = build;
  if (!chosen) {
    const promos = (await httpJson(
      "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json"
    )) as { promos: Record<string, string> };
    chosen = promos.promos[`${mc}-recommended`] ?? promos.promos[`${mc}-latest`];
    if (!chosen) throw new Error(`Forge no soporta ${mc}`);
  }
  const base = `https://maven.minecraftforge.net/net/minecraftforge/forge/${mc}-${chosen}`;
  return {
    url: `${base}/forge-${mc}-${chosen}-installer.jar`,
    filename: `forge-installer-${mc}.jar`,
    installer: true,
    resolved: chosen,
  };
}

// --- NeoForge (1.20.2+; versión = prefijo de MC sin el "1.") ---

export async function neoforgeBuilds(mc: string): Promise<BuildInfo[]> {
  const prefix = mc.split(".").slice(1).join(".");
  const data = (await httpJson(
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge"
  )) as { versions: string[] };
  return data.versions
    .filter((v) => v.startsWith(`${prefix}.`))
    .reverse()
    .map((id) => ({ id }));
}

export async function neoforgeDownload(mc: string, build?: string): Promise<EngineDownload> {
  const prefix = mc.split(".").slice(1).join(".");
  let chosen = build;
  if (!chosen) {
    const data = (await httpJson(
      "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge"
    )) as { versions: string[] };
    const candidates = data.versions.filter((v) => v.startsWith(`${prefix}.`));
    if (candidates.length === 0) throw new Error(`NeoForge no soporta ${mc}`);
    chosen = candidates[candidates.length - 1];
  }
  const base = `https://maven.neoforged.net/releases/net/neoforged/neoforge/${chosen}`;
  return {
    url: `${base}/neoforge-${chosen}-installer.jar`,
    filename: `neoforge-installer-${mc}.jar`,
    installer: true,
    resolved: chosen,
  };
}
