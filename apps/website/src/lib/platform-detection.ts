import type { OperatingSystem } from "../data/releases";

export interface PlatformHints {
  userAgentDataPlatform?: string;
  userAgentDataMobile?: boolean;
  navigatorPlatform?: string;
  userAgent: string;
  maxTouchPoints?: number;
}

export function detectDesktopOperatingSystem(hints: PlatformHints): OperatingSystem | null {
  const userAgent = hints.userAgent.toLowerCase();
  const platform = (
    hints.userAgentDataPlatform ??
    hints.navigatorPlatform ??
    hints.userAgent
  ).toLowerCase();
  const mobileLike =
    hints.userAgentDataMobile === true ||
    /android|iphone|ipad|ipod|mobile|windows phone/.test(userAgent) ||
    /android|iphone|ipad|ipod/.test(platform) ||
    (platform.includes("mac") && (hints.maxTouchPoints ?? 0) > 1);

  if (mobileLike || userAgent.includes("cros")) return null;
  if (platform.includes("win")) return "windows";
  if (platform.includes("mac")) return "macos";
  if (platform.includes("linux")) return "linux";
  return null;
}
