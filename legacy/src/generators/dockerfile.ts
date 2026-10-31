/** El Dockerfile del proyecto (paridad con rust/crates/forge-core/src/generators/dockerfile.rs). */
import type { DownloadPlan } from "../plan.js";
import type { ServerConfig } from "../types.js";
import { javaMajorFor } from "../catalog.js";

export function dockerfile(cfg: ServerConfig, _plan: DownloadPlan): string {
  const { name, engine, mcVersion } = cfg;
  const java = javaMajorFor(mcVersion);
  const puerto = cfg.port;
  const rcon = cfg.rcon.port;
  return `# OpenBlock — ${name} (${engine} ${mcVersion})
# build:  docker build -t ${name} .
# run:    docker run -d --name ${name} -p ${puerto}:${puerto} -p ${rcon}:${rcon} -v ${name}-data:/server/world --restart unless-stopped ${name}
# (el -v conserva el mundo entre recreaciones del contenedor)
FROM eclipse-temurin:${java}-jre

WORKDIR /server
COPY . .
RUN chmod +x start.sh install.sh

EXPOSE ${puerto}/tcp
EXPOSE ${rcon}/tcp

# start.sh ya lleva el bucle anti-crash; --restart unless-stopped cubre el resto
ENTRYPOINT ["./start.sh"]
`;
}
