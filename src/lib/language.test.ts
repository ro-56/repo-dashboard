import { get } from "svelte/store";
import { locale as i18nLocale } from "svelte-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";

const localeMock = vi.fn();
vi.mock("@tauri-apps/plugin-os", () => ({ locale: localeMock }));

const { getLanguage, initLanguage, setLanguage } = await import("./language");

function fakeStorage(initial: Record<string, string> = {}) {
  const store = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => {
      store.set(key, value);
    },
    removeItem: (key: string) => {
      store.delete(key);
    },
  };
}

function stubEnv(options: { stored?: string; navigatorLanguage?: string } = {}) {
  vi.stubGlobal("localStorage", fakeStorage(options.stored ? { language: options.stored } : {}));
  vi.stubGlobal("navigator", { language: options.navigatorLanguage ?? "" });
}

afterEach(() => {
  vi.unstubAllGlobals();
  localeMock.mockReset();
});

describe("initLanguage", () => {
  it("reads a stored choice from localStorage without consulting the OS locale", async () => {
    stubEnv({ stored: "pt-BR", navigatorLanguage: "en-US" });

    expect(await initLanguage()).toBe("pt-BR");
    expect(localeMock).not.toHaveBeenCalled();
    expect(getLanguage()).toBe("pt-BR");
  });

  it("an explicit stored choice is never overridden by a later OS-locale change", async () => {
    stubEnv({ stored: "en", navigatorLanguage: "en-US" });
    await initLanguage();

    // Simulate the OS locale changing after startup — nothing re-reads it.
    vi.stubGlobal("navigator", { language: "pt-BR" });

    expect(getLanguage()).toBe("en");
    expect(localeMock).not.toHaveBeenCalled();
  });

  it("falls back to the OS-detected language when nothing is stored and it's supported", async () => {
    stubEnv({ navigatorLanguage: "pt-BR" });

    expect(await initLanguage()).toBe("pt-BR");
  });

  it("falls back to English when nothing is stored and the OS locale is unsupported", async () => {
    stubEnv({ navigatorLanguage: "fr-FR" });
    localeMock.mockResolvedValue("fr-FR");

    expect(await initLanguage()).toBe("en");
  });
});

describe("setLanguage", () => {
  it("persists the choice to localStorage and updates the active catalog", async () => {
    stubEnv({ stored: "en" });

    setLanguage("pt-BR");

    expect(getLanguage()).toBe("pt-BR");
    expect(localStorage.getItem("language")).toBe("pt-BR");
    expect(get(i18nLocale)).toBe("pt-BR");
  });
});
