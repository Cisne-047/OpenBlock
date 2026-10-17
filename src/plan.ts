import { engineById } from "./catalog.js";
import {
  fabricDownload,
  forgeDownload,
  neoforgeDownload,
  paperFamilyDownload,
  quiltDownload,
} from "./api/engines.js";
import { vanillaServerFor } from "./api/mojang.js";
import {
  curseforgeFileUrl,
  latestVersion,
  mrpackServerFiles,
  parseMrpackIndex,
  primaryFile,
  type MrpackIndex,
} from "./api/modrinth.js";
import { perfItemsFor } from "./catalog.js";
import type { ServerConfig } from "./types.js";

/**
 * Manifiesto de descargas: lo resuelven las APIs ANTES de generar el proyecto,
 * así apply (local) y los install.sh/ps1 del zip embenido usan las mismas URLs
 * y hashes. OpenBlock genera el plan una vez y lo ejecuta en cualquier SO.
 */

export type DownloadKind = "engine" | "plugin" | "mod" | "modpack-file";

export interface DownloadEntry {
  kind: DownloadKind;
  label: string;
  url: string;
  /** ruta relativa dentro del server; "" para CF: se deduce del nombre en la URL */
  dest: string;
  sha256?: string;
  sha1?: string;
  sha512?: string;
}

export interface DownloadPlan {
  entries: DownloadEntry[];
  /** jar installer que hay que ejecutar con java antes del primer arranque */
  installer?: { jar: string; engine: "forge" | "neoforge" };
  engineFilename: string;
  engineResolved: string;
  /** modpacks CurseForge sin API key: pendientes de resolver a mano */
  unresolvedCf?: number;
  /** loader version detectada (fabric/quilt) para trazabilidad */
  loaderResolved?: string;
  /** ficheros locales del usuario a copiar al server */
  localCopies: Array<{ src: string; dest: string }>;
}

/** loader tag de Modrinth para pedir versiones de plugins/mods */
function modrinthLoader(cfg: ServerConfig): string | null {
  switch (cfg.engine) {
    case "paper":
      return "paper";
    case "purpur":
      return "paper";
    case "folia":
      return "folia";
    case "fabric":
      return "fabric";
    case "quilt":
      return "quilt";
    case "forge":
      return "forge";
    case "neoforge":
      return "neoforge";
    default:
      return null;
  }
}

export async function buildDownloadPlan(cfg: ServerConfig): Promise<DownloadPlan> {
  const entries: DownloadEntry[] = [];
  const localCopies: Array<{ src: string; dest: string }> = [];
  const engine = engineById(cfg.engine);
  let plan: Omit<DownloadPlan, "entries" | "localCopies">;

  switch (cfg.engine) {
    case "vanilla": {
      const dl = await vanillaServerFor(cfg.mcVersion);
      plan = { engineFilename: "server.jar", engineResolved: cfg.mcVersion };
      entries.push({ kind: "engine", label: `Vanilla ${cfg.mcVersion}`, url: dl.url, dest: "server.jar" });
      break;
    }
    case "paper":
    case "purpur":
    case "folia": {
      const dl = await paperFamilyDownload(cfg.engine, cfg.mcVersion, cfg.build);
      plan = { engineFilename: dl.filename, engineResolved: dl.resolved };
      entries.push({ kind: "engine", label: `${engine.label} ${cfg.mcVersion} (${dl.resolved})`, url: dl.url, dest: dl.filename, sha256: dl.sha256 });
      break;
    }
    case "fabric":
    case "quilt": {
      const dl =
        cfg.engine === "fabric"
          ? await fabricDownload(cfg.mcVersion, cfg.loaderBuild, cfg.installerBuild)
          : await quiltDownload(cfg.mcVersion, cfg.loaderBuild, cfg.installerBuild);
      plan = { engineFilename: dl.filename, engineResolved: dl.resolved, loaderResolved: dl.resolved };
      entries.push({ kind: "engine", label: `${engine.label} ${cfg.mcVersion} (${dl.resolved})`, url: dl.url, dest: dl.filename });
      break;
    }
    case "forge":
    case "neoforge": {
      const dl = cfg.engine === "forge" ? await forgeDownload(cfg.mcVersion, cfg.build) : await neoforgeDownload(cfg.mcVersion, cfg.build);
      plan = {
        engineFilename: dl.filename,
        engineResolved: dl.resolved,
        installer: { jar: dl.filename, engine: cfg.engine },
      };
      entries.push({ kind: "engine", label: `${engine.label} ${cfg.mcVersion} (${dl.resolved})`, url: dl.url, dest: dl.filename });
      break;
    }
  }

  const mrLoader = modrinthLoader(cfg);
  const mcFilter = [cfg.mcVersion];

  // plugins/mods del catálogo de rendimiento (chunkyborder solo si se pide)
  if (mrLoader) {
    for (const item of perfItemsFor(engine.loader, cfg.mcVersion)) {
      if (!item.default && !(item.id === "chunkyborder" && cfg.chunkyborder)) continue;
      try {
        const v = await latestVersion(item.slug, { gameVersions: mcFilter, loaders: [mrLoader] });
        const f = primaryFile(v);
        const subdir = item.kind === "plugin" ? "plugins" : "mods";
        entries.push({
          kind: item.kind,
          label: item.label,
          url: f.url,
          dest: `${subdir}/${f.filename}`,
          sha512: f.hashes?.sha512,
        });
      } catch (err) {
        // item sin build para esa versión: se salta con aviso en el plan
        plan.unresolvedCf ??= 0;
        console.warn(`[plan] ${item.label}: sin build para ${cfg.mcVersion} (${(err as Error).message})`);
      }
    }
  }

  // plugins/mods propios del usuario (URLs directas; los locales los copia apply)
  for (const item of cfg.customItems ?? []) {
    const subdir = item.kind === "plugin" ? "plugins" : "mods";
    if (item.url) {
      entries.push({
        kind: item.kind,
        label: `${item.name} (custom)`,
        url: item.url,
        dest: `${subdir}/${item.name}${item.name.endsWith(".jar") ? "" : ".jar"}`,
      });
    } else if (item.path) {
      localCopies.push({ src: item.path, dest: `${subdir}/${item.name}${item.name.endsWith(".jar") ? "" : ".jar"}` });
    }
  }

  // modpacks
  if (cfg.modpack.kind === "modrinth") {
    const mrpack = await fetchMrpack(cfg.modpack.url);
    plan.engineResolved += ` + modpack ${mrpack.index.name} ${mrpack.index.versionId}`;
    for (const f of mrpackServerFiles(mrpack.index)) {
      entries.push({
        kind: "modpack-file",
        label: f.path,
        url: f.downloads[0],
        dest: f.path,
        sha512: f.hashes.sha512,
      });
    }
  } else if (cfg.modpack.kind === "curseforge") {
    // los overrides/mods van dentro del zip que baja el usuario; aquí solo
    // registramos la intención: el manifest se resuelve con OPENBLOCK_CF_KEY
    plan.unresolvedCf = -1;
  }

  return { ...plan, entries, localCopies };
}

interface MrpackFetch {
  index: MrpackIndex;
}

/** descarga y parsea el índice de un .mrpack (URL de página de Modrinth o directa) */
export async function fetchMrpack(url: string): Promise<MrpackFetch> {
  const { modrinthModpackFile } = await import("./api/modrinth.js");
  const { file } = await modrinthModpackFile(url);
  const res = await fetch(file.url);
  if (!res.ok) throw new Error(`HTTP ${res.status} bajando ${file.filename}`);
  const JSZip = (await import("jszip")).default;
  const zip = await JSZip.loadAsync(Buffer.from(await res.arrayBuffer()));
  const indexFile = zip.file("modrinth.index.json");
  if (!indexFile) throw new Error(`${file.filename} no es un modpack Modrinth (.mrpack) válido`);
  const index = parseMrpackIndex(await indexFile.async("string"));
  return { index };
}

/** resuelve las URLs de un manifest CurseForge (requiere OPENBLOCK_CF_KEY) */
export async function resolveCurseforgeEntries(manifestJson: string): Promise<DownloadEntry[]> {
  const manifest = JSON.parse(manifestJson);
  const out: DownloadEntry[] = [];
  for (const f of manifest.files) {
    if (!f.required) continue;
    out.push({
      kind: "modpack-file",
      label: `CF ${f.projectID}/${f.fileID}`,
      url: await curseforgeFileUrl(f.projectID, f.fileID),
      dest: "", // el nombre real lo dice la API de descarga
    });
  }
  return out;
}
