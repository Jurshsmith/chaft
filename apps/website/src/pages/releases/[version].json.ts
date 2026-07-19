import { allReleases, type ReleaseManifest } from "../../data/releases";

export const prerender = true;

export function getStaticPaths() {
  return allReleases.map((release) => ({
    params: { version: release.version },
    props: { release },
  }));
}

interface Context {
  props: {
    release: ReleaseManifest;
  };
}

export function GET({ props }: Context) {
  return new Response(`${JSON.stringify(props.release, null, 2)}\n`, {
    headers: {
      "Content-Type": "application/json; charset=utf-8",
    },
  });
}
