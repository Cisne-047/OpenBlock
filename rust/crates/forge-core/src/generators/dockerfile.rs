//! El Dockerfile del proyecto (`dockerfile`).
//!
//! Paridad con DistroForge, que también genera OCI: aquí la imagen es un
//! wrapper del server ya instalado — `COPY . .` del directorio (o del zip
//! exportado) y el `start.sh` de siempre dentro de un Temurin del Java que
//! toca. El bucle anti-crash del script se convierte en la política de
//! reinicio del contenedor de regalo: `docker run --restart unless-stopped`.

use crate::catalog::java_major_for;
use crate::plan::DownloadPlan;
use crate::types::ServerConfig;

pub fn dockerfile(cfg: &ServerConfig, _plan: &DownloadPlan) -> String {
    let (name, engine, mc) = (&cfg.name, cfg.engine, &cfg.mc_version);
    let java = java_major_for(mc);
    let (puerto, rcon) = (cfg.port, cfg.rcon.port);
    format!(
        r#"# OpenBlock — {name} ({engine} {mc})
# build:  docker build -t {name} .
# run:    docker run -d --name {name} -p {puerto}:{puerto} -p {rcon}:{rcon} -v {name}-data:/server/world --restart unless-stopped {name}
# (el -v conserva el mundo entre recreaciones del contenedor)
FROM eclipse-temurin:{java}-jre

WORKDIR /server
COPY . .
RUN chmod +x start.sh install.sh

EXPOSE {puerto}/tcp
EXPOSE {rcon}/tcp

# start.sh ya lleva el bucle anti-crash; --restart unless-stopped cubre el resto
ENTRYPOINT ["./start.sh"]
"#
    )
}
