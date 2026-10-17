import { mkdtemp, open, rm } from "node:fs/promises";
import os from "node:os";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Worker } from "node:worker_threads";
import type { BenchmarkResult } from "../types";

/**
 * Benchmark rápido (~4s): score de CPU multithread con worker por core y
 * prueba de escritura secuencial de 128 MB para clasificar el disco.
 */

const WORKER_SRC = `
const { parentPort } = require("node:worker_threads");
const deadline = Date.now() + 1800;
let ops = 0n;
let x = 3n;
while (Date.now() < deadline) {
  for (let i = 0; i < 20000; i++) {
    // xorshift de enteros: mezcla de mul/add/shift, branchless
    x ^= x << 13n; x ^= x >> 7n; x ^= x << 17n;
    if ((x & 1n) === 1n) x += 0x9e3779b97f4a7c15n;
    ops += 2n;
  }
}
parentPort.postMessage(ops.toString());
`;

async function workerScore(): Promise<number> {
  const cores = Math.max(1, Math.min(32, os.cpus().length));
  const workers = Array.from({ length: cores }, () => {
    const w = new Worker(WORKER_SRC, { eval: true });
    return {
      w,
      ops: new Promise<bigint>((res) =>
        w.once("message", (m) => res(BigInt(m as string)))
      ),
    };
  });
  const t0 = Date.now();
  const results = await Promise.all(workers.map((s) => s.ops));
  workers.forEach((s) => s.w.terminate());
  const elapsed = Math.max(1, Date.now() - t0);
  const total = results.reduce((a, b) => a + b, 0n);
  return Math.round(Number(total / BigInt(cores)) / elapsed);
}

export async function runBenchmark(): Promise<BenchmarkResult> {
  const t0 = Date.now();
  const [cpuScore, diskMBs] = await Promise.all([workerScore(), diskWriteTest()]);
  return { cpuScore, diskMBs, durationMs: Date.now() - t0 };
}

async function diskWriteTest(): Promise<number> {
  const dir = await mkdtemp(join(tmpdir(), "openblock-bench-"));
  try {
    const file = join(dir, "seq.bin");
    const CHUNK = 4 * 1024 * 1024;
    const TOTAL = 128 * 1024 * 1024;
    const buf = Buffer.alloc(CHUNK, 0x5a);
    const fh = await open(file, "w");
    const t0 = Date.now();
    for (let off = 0; off < TOTAL; off += CHUNK) await fh.write(buf, 0, CHUNK, off);
    await fh.sync();
    await fh.close();
    const secs = (Date.now() - t0) / 1000;
    return Math.max(1, Math.round(TOTAL / 1048576 / secs));
  } catch {
    return 0;
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}
