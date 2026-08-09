import { describe, expect, it } from "vitest";

import { currentRelease } from "../data/releases";
import {
  buildDownloadPlatforms,
  formatArtifactLabel,
  formatVariantLabel,
} from "./download-options";

describe("download options", () => {
  it("groups the current release into three real desktop platforms", () => {
    const platforms = buildDownloadPlatforms(currentRelease);

    expect(platforms.map(({ id }) => id)).toEqual([
      "windows",
      "macos",
      "linux",
    ]);
    expect(platforms.find(({ id }) => id === "windows")?.options).toHaveLength(1);
    expect(platforms.find(({ id }) => id === "macos")?.options).toHaveLength(2);
    expect(platforms.find(({ id }) => id === "linux")?.options).toHaveLength(1);
  });

  it("orders and labels the two published macOS architectures explicitly", () => {
    const macos = buildDownloadPlatforms(currentRelease).find(
      ({ id }) => id === "macos",
    );

    expect(macos?.options.map(({ variantLabel }) => variantLabel)).toEqual([
      "Apple Silicon · arm64",
      "Intel · x86_64",
    ]);
    expect(macos?.options.every(({ formatLabel }) => formatLabel === "DMG")).toBe(
      true,
    );
  });

  it("keeps architecture and format labels honest for unknown values", () => {
    const windows = currentRelease.assets.find(({ os }) => os === "windows");
    expect(windows).toBeDefined();
    expect(formatVariantLabel(windows!)).toBe("x86_64");
    expect(formatArtifactLabel("appimage")).toBe("AppImage");
    expect(formatArtifactLabel("pkg")).toBe("pkg");
  });

  it("prefers an available macOS artifact when another variant is pending", () => {
    const releaseWithPendingArm = {
      version: currentRelease.version,
      assets: currentRelease.assets.map((asset) =>
        asset.id === "macos-arm64-dmg"
          ? { ...asset, available: false }
          : asset,
      ),
    };

    const macos = buildDownloadPlatforms(releaseWithPendingArm).find(
      ({ id }) => id === "macos",
    );
    expect(macos?.options.map(({ asset }) => [asset.arch, asset.available])).toEqual([
      ["x86_64", true],
      ["arm64", false],
    ]);
  });
});
