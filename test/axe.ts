import { run } from "axe-core";
import { expect } from "vitest";

/**
 * Assert an element has no axe accessibility violations.
 *
 * `color-contrast` is disabled because jsdom has no layout engine to compute it. The design-token
 * contrast test covers the application's supported text/background pairings in both themes.
 */
export async function expectNoA11yViolations(container: HTMLElement): Promise<void> {
  const results = await run(container, {
    runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"] },
    resultTypes: ["violations"],
    rules: { "color-contrast": { enabled: false } },
  });
  const summary = results.violations.map((v) => `${v.id}: ${v.help}`).join("\n");
  expect(results.violations, summary).toEqual([]);
}
