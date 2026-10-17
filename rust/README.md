# OpenBlock en Rust

Port del núcleo a Rust, **en curso**. El TypeScript sigue siendo la
implementación de referencia y se queda en `src/` como espejo, igual que
DistroForge mantiene su `legacy/`.

## Reparto

```
crates/forge-core   núcleo generador: puro y sin E/S
crates/forge-cli    CLI (en construcción)
```

`forge-core` no tiene una sola referencia a `std::fs`, `std::io`, `std::process`
ni `std::env`: construir contenido es puro y toda la E/S vive en el borde. No es
una convención escrita en un documento, es la misma que el TS cumple a rajatabla
desde que se sacó la E/S fuera de `generators/`, y `tests/purity.test.ts` la
vigila al otro lado.

## Estado

| módulo | estado |
|---|---|
| `types` | portado (orden de campos idéntico al TS) |
| `generators::jvm` | portado |
| `generators::properties` | portado |
| `generators::paper` | portado |
| `generators::ops` | portado |
| `generators::service` | portado |
| `catalog` | pendiente |
| `generators::scripts` | pendiente |
| `generators` (raíz, `generateProject`) | pendiente |
| `hw` (detección y tiers) | pendiente |
| `plan` y `api` | pendiente |

## Paridad

El núcleo Rust se compara **byte a byte** contra el TypeScript. El golden lo
genera el TS y lo consume Rust:

```bash
npm run parity          # regenera el golden y corre los tests de Rust
cargo test              # solo los tests, con el golden ya generado
```

Es el `diff -r` que DistroForge documenta como procedimiento manual, pero
automatizado y dentro de la suite: si alguien cambia un generador en un lado y no
en el otro, `cargo test` falla diciendo **el fichero, la línea y el byte**:

```
paridad rota en «jvm-args.txt» (línea 1, byte 305)
  TS     (1): "…G1ReservePercent=20…"
  Rust   (1): "…G1ReservePercent=21…"
  longitudes: TS 612 / Rust 612
```

El CI regenera el golden y comprueba con `git diff --exit-code` que está al día,
así que la paridad no puede derivar en silencio.

## Divergencias conocidas

Deliberadas y anotadas, como la lista que DistroForge mantiene en su
`rust/README.md`:

1. **Enums cerrados donde el TS tiene uniones de strings.** Un engine inventado
   no compila ni deserializa, en vez de propagarse hasta el generador y fallar
   allí. El valor en JSON es idéntico.
2. **`CustomItem.url` y `path` siempre se serializan.** En el TS, un item de URL
   se construye con spread y **no lleva la clave `path`**, mientras que uno local
   no lleva `url`; aquí los dos van y el ausente sale `null`. Solo afecta a
   `customItems` en `openblock.json`, no a ningún fichero generado.
3. **`diskMBs` y `ramMB` llevan `#[serde(rename)]` explícito.** El `camelCase` de
   serde convierte `disk_m_bs` en `diskMbS`, que no es lo que dice el JSON del TS:
   serde no adivina acrónimos, así que se declaran a mano.

## Por qué Rust

No arregla el reparto —ese problema ya está resuelto publicando en npm, y
`cargo install` sería *peor* para el usuario porque le obligaría a tener Rust—.
Se hace porque un binario único sin dependencia de Node es un final mejor para
una CLI que la gente va a ejecutar en su máquina, y porque el núcleo ya es puro y
tiene 86 tests como contrato. El reparto en npm sigue funcionando durante todo el
port: no hay una fase en la que OpenBlock esté roto.
