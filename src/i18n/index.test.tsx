import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { I18nProvider, useI18n } from "./index";

function Probe(props: { keyName: string; count?: number; name?: string }) {
  const { t, setAppLanguage } = useI18n();
  return (
    <div>
      <button onClick={() => setAppLanguage("de")}>de</button>
      <span data-testid="value">
        {t(props.keyName, { count: props.count, name: props.name })}
      </span>
    </div>
  );
}

describe("i18n", () => {
  it("resolves english text by default", () => {
    render(
      <I18nProvider>
        <Probe keyName="test.hello" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("value").textContent).toBe("Hello");
  });

  it("switches locale and falls back to english when key missing", () => {
    render(
      <I18nProvider>
        <Probe keyName="test.only_en" />
      </I18nProvider>,
    );
    screen.getByText("de").click();
    expect(screen.getByTestId("value").textContent).toBe("Only English");
  });

  it("supports interpolation and pluralization", () => {
    render(
      <I18nProvider>
        <Probe keyName="test.count" count={2} name="Sam" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("value").textContent).toBe("Sam has 2 steps");
  });
});
