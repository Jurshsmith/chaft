import { describe, expect, it } from "vitest";

import { detectDesktopOperatingSystem } from "./platform-detection";

describe("detectDesktopOperatingSystem", () => {
  it.each([
    ["Win32", "windows"],
    ["MacIntel", "macos"],
    ["Linux x86_64", "linux"],
  ] as const)("detects desktop platform %s", (navigatorPlatform, expected) => {
    expect(
      detectDesktopOperatingSystem({ navigatorPlatform, userAgent: navigatorPlatform }),
    ).toBe(expected);
  });

  it("does not recommend a Linux desktop artifact to Android", () => {
    expect(
      detectDesktopOperatingSystem({
        navigatorPlatform: "Linux armv8l",
        userAgent: "Mozilla/5.0 (Linux; Android 16; Mobile)",
      }),
    ).toBeNull();
  });

  it("does not recommend a macOS artifact to touch-based iPadOS", () => {
    expect(
      detectDesktopOperatingSystem({
        navigatorPlatform: "MacIntel",
        userAgent: "Mozilla/5.0 AppleWebKit Safari",
        maxTouchPoints: 5,
      }),
    ).toBeNull();
  });

  it("does not treat ChromeOS as desktop Linux", () => {
    expect(
      detectDesktopOperatingSystem({
        navigatorPlatform: "Linux x86_64",
        userAgent: "Mozilla/5.0 (X11; CrOS x86_64)",
      }),
    ).toBeNull();
  });
});
