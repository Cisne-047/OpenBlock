import { describe, expect, it } from "vitest";
import { evaluateSelfTest, selfTestConfig } from "../src/selftest";
import type { DoctorReport } from "../src/doctor";

/**
 * El veredicto del self-test es lo único que decide si CI pasa o falla, así que
 * se prueba el caso malo explícitamente. Un verificador del que solo se comprueba
 * el "OK" no verifica nada: por eso el bug de `--yes` se coló entero.
 */

const report = (over: Partial<DoctorReport> = {}): DoctorReport => ({
  findings: [],
  applied: [],
  pending: [],
  score: 100,
  grade: "SANO",
  bootTested: true,
  bootOk: true,
  ...over,
});

const fallos = (r: DoctorReport) => evaluateSelfTest(r).filter((s) => !s.ok).map((s) => s.name);

describe("evaluateSelfTest", () => {
  it("server sano: todo en verde", () => {
    expect(fallos(report())).toEqual([]);
  });

  it("el server no arranca: falla el arranque", () => {
    expect(fallos(report({ bootOk: false }))).toContain("arranque real (Done)");
  });

  it("hay un finding: falla, y dice cuál", () => {
    // Un finding en un server recién instalado es un bug: o el generador escribe
    // algo mal o el doctor tiene un falso positivo. El self-test no distingue
    // quién tiene la culpa, solo que algo va mal.
    const r = report({
      findings: [{ severity: "error", code: "ENGINE_JAR_MISSING", message: "falta el jar", source: "x" }],
      score: 80,
    });
    const pasos = evaluateSelfTest(r);
    expect(pasos.find((s) => s.name === "sin findings")?.ok).toBe(false);
    expect(pasos.find((s) => s.name === "sin findings")?.detail).toBe("ENGINE_JAR_MISSING");
    expect(fallos(r)).toContain("sin findings");
    expect(fallos(r)).toContain("score 100");
  });

  it("el doctor escribió algo en solo lectura: falla", () => {
    expect(fallos(report({ applied: ["accept-eula"] }))).toContain("solo lectura respetada");
  });

  it("score por debajo de 100 sin findings: falla igual", () => {
    // no debería pasar (el score sale de los findings), pero si pasa es un bug
    // del cálculo y el self-test tiene que verlo
    expect(fallos(report({ score: 95 }))).toContain("score 100");
  });
});

describe("selfTestConfig", () => {
  it("no pregenera ni deja servicios: mide arranque, no rendimiento", () => {
    const cfg = selfTestConfig("/tmp/x", { cores: 8 });
    expect(cfg.pregenRadius).toBe(0);
    expect(cfg.service.enabled).toBe(false);
    expect(cfg.backup.enabled).toBe(false);
  });

  it("usa el puerto pedido y aparta el RCON del de juego", () => {
    const cfg = selfTestConfig("/tmp/x", { cores: 8, port: 26000 });
    expect(cfg.port).toBe(26000);
    expect(cfg.rcon.port).not.toBe(cfg.port);
  });

  it("siempre lleva contraseña de RCON (el doctor la exige para apagar limpio)", () => {
    expect(selfTestConfig("/tmp/x", { cores: 8 }).rcon.password.length).toBeGreaterThanOrEqual(8);
  });

  it("respeta la versión de MC pedida", () => {
    expect(selfTestConfig("/tmp/x", { cores: 8, mc: "1.21.4" }).mcVersion).toBe("1.21.4");
    expect(selfTestConfig("/tmp/x", { cores: 8 }).mcVersion).toBe("1.20.4");
  });
});
