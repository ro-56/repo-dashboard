import { afterEach, describe, expect, it, vi } from "vitest";
import { getTheme, initTheme, setTheme } from "./theme";

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

function stubEnv(options: { stored?: string; prefersDark?: boolean } = {}) {
  const setAttribute = vi.fn();
  const matchMedia = vi.fn().mockReturnValue({ matches: options.prefersDark ?? false });
  vi.stubGlobal("localStorage", fakeStorage(options.stored ? { theme: options.stored } : {}));
  vi.stubGlobal("document", { documentElement: { setAttribute } });
  vi.stubGlobal("matchMedia", matchMedia);
  return { setAttribute, matchMedia };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("initTheme", () => {
  it("reads a stored choice from localStorage without consulting prefers-color-scheme", () => {
    const { matchMedia, setAttribute } = stubEnv({ stored: "dark", prefersDark: false });

    expect(initTheme()).toBe("dark");
    expect(matchMedia).not.toHaveBeenCalled();
    expect(setAttribute).toHaveBeenCalledWith("data-theme", "dark");
    expect(getTheme()).toBe("dark");
  });

  it("an explicit stored choice is never overridden by a later OS-preference change", () => {
    const { matchMedia } = stubEnv({ stored: "light", prefersDark: true });

    expect(initTheme()).toBe("light");
    expect(matchMedia).not.toHaveBeenCalled();
  });

  it("falls back to prefers-color-scheme: dark when nothing is stored", () => {
    stubEnv({ prefersDark: true });

    expect(initTheme()).toBe("dark");
  });

  it("falls back to prefers-color-scheme: light when nothing is stored", () => {
    stubEnv({ prefersDark: false });

    expect(initTheme()).toBe("light");
  });
});

describe("setTheme", () => {
  it("persists the choice to localStorage and applies it via data-theme", () => {
    const { setAttribute } = stubEnv({ stored: "light" });

    setTheme("dark");

    expect(getTheme()).toBe("dark");
    expect(setAttribute).toHaveBeenCalledWith("data-theme", "dark");
    expect(localStorage.getItem("theme")).toBe("dark");
  });
});
