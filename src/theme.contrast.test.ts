import { describe, expect, it } from "vitest";
import { wcagContrast } from "culori";

// Design tokens from src/index.css. Verifies every text/background pair meets WCAG 2.1 AA
// contrast (≥4.5:1 for normal text) in BOTH themes — the check jsdom + axe cannot perform.
const T = {
  light: {
    background: "oklch(1 0 0)",
    foreground: "oklch(0.145 0 0)",
    primary: "oklch(0.546 0.182 256)",
    primaryFg: "oklch(0.985 0 0)",
    secondary: "oklch(0.97 0 0)",
    secondaryFg: "oklch(0.205 0 0)",
    mutedFg: "oklch(0.556 0 0)",
    destructive: "oklch(0.577 0.245 27.325)",
    destructiveFg: "oklch(0.985 0 0)",
    success: "oklch(0.5 0.14 150)",
    white: "#ffffff",
  },
  dark: {
    background: "oklch(0.145 0 0)",
    foreground: "oklch(0.985 0 0)",
    primary: "oklch(0.685 0.169 256)",
    primaryFg: "oklch(0.145 0 0)",
    secondary: "oklch(0.269 0 0)",
    secondaryFg: "oklch(0.985 0 0)",
    mutedFg: "oklch(0.708 0 0)",
    destructive: "oklch(0.704 0.191 22.216)",
    destructiveFg: "oklch(0.145 0 0)",
    success: "oklch(0.5 0.14 150)",
    white: "#ffffff",
  },
};

const ratio = (a: string, b: string) => wcagContrast(a, b);
const AA_TEXT = 4.5;

describe("design token contrast (WCAG 2.1 AA, normal text ≥ 4.5:1)", () => {
  it("light — body text on background", () => {
    expect(ratio(T.light.foreground, T.light.background)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("light — muted text on background", () => {
    expect(ratio(T.light.mutedFg, T.light.background)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("light — primary button text", () => {
    expect(ratio(T.light.primaryFg, T.light.primary)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("light — secondary text", () => {
    expect(ratio(T.light.secondaryFg, T.light.secondary)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("light — destructive text on background (tinted badge)", () => {
    expect(ratio(T.light.destructive, T.light.background)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("light — success badge text", () => {
    expect(ratio(T.light.white, T.light.success)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("dark — success badge text", () => {
    expect(ratio(T.dark.white, T.dark.success)).toBeGreaterThanOrEqual(AA_TEXT);
  });

  it("dark — body text on background", () => {
    expect(ratio(T.dark.foreground, T.dark.background)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("dark — muted text on background", () => {
    expect(ratio(T.dark.mutedFg, T.dark.background)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("dark — primary button text", () => {
    expect(ratio(T.dark.primaryFg, T.dark.primary)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("dark — secondary text", () => {
    expect(ratio(T.dark.secondaryFg, T.dark.secondary)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("dark — destructive text on background (tinted badge)", () => {
    expect(ratio(T.dark.destructive, T.dark.background)).toBeGreaterThanOrEqual(AA_TEXT);
  });

  // Solid destructive buttons (confirm dialogs) — label on the destructive background.
  it("light — destructive button label", () => {
    expect(ratio(T.light.destructiveFg, T.light.destructive)).toBeGreaterThanOrEqual(AA_TEXT);
  });
  it("dark — destructive button label", () => {
    expect(ratio(T.dark.destructiveFg, T.dark.destructive)).toBeGreaterThanOrEqual(AA_TEXT);
  });
});
