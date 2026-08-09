import type {
  OperatingSystem,
  ReleaseAsset,
  ReleaseManifest,
} from "../data/releases";

const platformDetails = {
  windows: {
    code: "win",
    label: "Windows",
  },
  macos: {
    code: "mac",
    label: "macOS",
  },
  linux: {
    code: "linux",
    label: "Linux",
  },
} as const;

const formatLabels: Record<string, string> = {
  appimage: "AppImage",
  dmg: "DMG",
  exe: "EXE",
  msi: "MSI",
  tar: "TAR",
  tgz: "TGZ",
  zip: "ZIP",
};

export interface DownloadOption {
  asset: ReleaseAsset;
  accessibleName: string;
  formatLabel: string;
  variantLabel: string;
}

export interface DownloadPlatform {
  code: string;
  id: OperatingSystem;
  label: string;
  options: DownloadOption[];
}

export function formatArtifactLabel(format: string): string {
  return formatLabels[format.toLowerCase()] ?? format;
}

export function formatVariantLabel(asset: ReleaseAsset): string {
  if (asset.os === "macos") {
    if (asset.arch === "arm64") return "Apple Silicon · arm64";
    if (asset.arch === "x86_64") return "Intel · x86_64";
  }
  return asset.arch;
}

export function buildDownloadPlatforms(
  release: Pick<ReleaseManifest, "assets" | "version">,
): DownloadPlatform[] {
  const platformOrder: OperatingSystem[] = ["windows", "macos", "linux"];

  return platformOrder
    .map((id) => {
      const details = platformDetails[id];
      const assets = release.assets
        .filter((asset) => asset.os === id)
        .sort((left, right) => {
          if (left.available !== right.available) return left.available ? -1 : 1;
          if (id !== "macos") return left.arch.localeCompare(right.arch);
          if (left.arch === "arm64") return -1;
          if (right.arch === "arm64") return 1;
          return left.arch.localeCompare(right.arch);
        });

      return {
        ...details,
        id,
        options: assets.map((asset) => {
          const formatLabel = formatArtifactLabel(asset.format);
          return {
            asset,
            formatLabel,
            variantLabel: formatVariantLabel(asset),
            accessibleName: `Download Chaft ${release.version} for ${details.label} ${asset.arch} as ${formatLabel}`,
          };
        }),
      };
    })
    .filter((platform) => platform.options.length > 0);
}
