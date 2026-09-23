import { describe, expect, it } from "vitest";
import { converter, formatHex, parse, wcagContrast } from "culori";

import css from "./index.css?raw";

/*
 * Verifies the design tokens in src/index.css meet WCAG 2.1 AA in both themes — the check jsdom +
 * axe cannot perform. Tokens are read from the stylesheet itself, and translucent surfaces (tinted
 * badges and banners, overlays) are composited onto the surface they sit on before measuring.
 */

type Tokens = Record<string, string>;

function tokens(selector: string): Tokens {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const block = css.match(new RegExp(`(?:^|\\n)${escaped}\\s*\\{([^}]*)\\}`))?.[1];
  if (!block) throw new Error(`no ${selector} block in index.css`);
  const out: Tokens = {};
  for (const [, name, value] of block
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .matchAll(/--([\w-]+):\s*([^;]+);/g)) {
    out[name] = value.trim();
  }
  return out;
}

const rgb = converter("rgb");

/** `color` at `alpha` (times its own alpha) laid over the opaque `base`. */
function over(color: string, base: string, alpha = 1): string {
  const top = rgb(parse(color));
  const bottom = rgb(parse(base));
  if (!top || !bottom) throw new Error(`unparseable colour: ${color} / ${base}`);
  const a = (top.alpha ?? 1) * alpha;
  const mix = (t: number, b: number) => t * a + b * (1 - a);
  return formatHex({
    mode: "rgb",
    r: mix(top.r, bottom.r),
    g: mix(top.g, bottom.g),
    b: mix(top.b, bottom.b),
  });
}

const AA_TEXT = 4.5;
const AA_NON_TEXT = 3;

// [description, text or control colour, surface colour, minimum ratio]
function pairs(t: Tokens): [string, string, string, number][] {
  const surfaces: [string, string][] = [
    ["page", t.background],
    ["card", t.card],
  ];
  return [
    ["primary button label", t["primary-foreground"], t.primary, AA_TEXT],
    ["secondary label", t["secondary-foreground"], t.secondary, AA_TEXT],
    ["hover label", t["accent-foreground"], t.accent, AA_TEXT],
    ["destructive button label", t["destructive-foreground"], t.destructive, AA_TEXT],
    ["success badge", "#ffffff", t.success, AA_TEXT],
    ["brand label", t["brand-foreground"], t.brand, AA_TEXT],
    ["muted text on a muted surface", t["muted-foreground"], t.muted, AA_TEXT],
    ...surfaces.flatMap(([name, surface]): [string, string, string, number][] => [
      [`body text on ${name}`, t.foreground, surface, AA_TEXT],
      [`muted text on ${name}`, t["muted-foreground"], surface, AA_TEXT],
      [`link text on ${name}`, t.primary, surface, AA_TEXT],
      [`error text on ${name}`, t.destructive, surface, AA_TEXT],
      [
        `muted text on a primary/5 banner (${name})`,
        t["muted-foreground"],
        over(t.primary, surface, 0.05),
        AA_TEXT,
      ],
      [
        `muted text on a muted/50 row (${name})`,
        t["muted-foreground"],
        over(t.muted, surface, 0.5),
        AA_TEXT,
      ],
      [
        `error text on a destructive/10 badge (${name})`,
        t.destructive,
        over(t.destructive, surface, 0.1),
        AA_TEXT,
      ],
      [
        `body text on a warning/15 badge (${name})`,
        t.foreground,
        over(t.warning, surface, 0.15),
        AA_TEXT,
      ],
      [`form control outline on ${name}`, over(t.input, surface), surface, AA_NON_TEXT],
      [`focus ring on ${name}`, t.ring, surface, AA_NON_TEXT],
    ]),
  ];
}

describe.each([
  ["light", tokens(":root")],
  ["dark", { ...tokens(":root"), ...tokens(".dark") }],
])("%s theme contrast (WCAG 2.1 AA)", (_theme, t) => {
  it.each(pairs(t))("%s", (_name, foreground, background, minimum) => {
    expect(wcagContrast(foreground, background)).toBeGreaterThanOrEqual(minimum);
  });
});
