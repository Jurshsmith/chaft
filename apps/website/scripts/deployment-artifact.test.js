import { mkdtemp, mkdir, readFile, rm, symlink, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { promisify } from "node:util";
import { execFile } from "node:child_process";

import { afterEach, describe, expect, it } from "vitest";

import {
  ARTIFACT_MANIFEST,
  ASSET_ROOT,
  DEPLOYMENT_MARKER,
  compareInstalledDeploymentArtifact,
  createDeploymentArtifact,
  deploymentMarkerPath,
  deploymentMountPath,
  installDeploymentArtifact,
  renderDeploymentMarker,
  validatePortablePath,
  verifyDeploymentArtifact,
} from "./deployment-artifact.mjs";

const identity = {
  sourceRepository: "Jurshsmith/chaft",
  sourceCommit: "0123456789abcdef0123456789abcdef01234567",
  siteUrl: "https://chaft.example",
};

const mountedIdentities = [
  {
    name: "one-segment",
    identity: { ...identity, siteUrl: "https://chaft.example/chaft/" },
    mountPath: "chaft",
  },
  {
    name: "multi-segment",
    identity: { ...identity, siteUrl: "https://chaft.example/products/chaft" },
    mountPath: "products/chaft",
  },
];

const temporaryRoots = [];
const execFileAsync = promisify(execFile);

async function temporaryRoot() {
  const root = await mkdtemp(join(tmpdir(), "chaft-website-artifact-"));
  temporaryRoots.push(root);
  return root;
}

async function writeFixture(source, siteUrl = identity.siteUrl) {
  const mountPath = deploymentMountPath(siteUrl);
  const mountedSource = mountPath
    ? join(source, ...mountPath.split("/"))
    : source;
  await mkdir(join(mountedSource, "_astro"), { recursive: true });
  await mkdir(join(mountedSource, ".metadata"), { recursive: true });
  await writeFile(join(mountedSource, "404.html"), "<h1>Not found</h1>\n");
  await writeFile(join(source, "_headers"), "/*\n  X-Frame-Options: DENY\n");
  await writeFile(join(source, "_redirects"), "/downloads /download/ 301\n");
  await writeFile(join(mountedSource, "index.html"), "<h1>Chaft</h1>\n");
  await writeFile(
    join(mountedSource, "_astro", "app.AbCd1234.js"),
    "export default 1;\n",
  );
  await writeFile(join(mountedSource, ".metadata", "empty"), "");
  await writeFile(join(mountedSource, "logo.bin"), Buffer.from([0, 1, 2, 255]));
}

async function fixture(siteIdentity = identity) {
  const root = await temporaryRoot();
  const source = join(root, "dist");
  const artifact = join(root, "artifact");
  await mkdir(source);
  await writeFixture(source, siteIdentity.siteUrl);
  return { root, source, artifact };
}

async function manifestAt(artifact) {
  return JSON.parse(await readFile(join(artifact, ARTIFACT_MANIFEST), "utf8"));
}

afterEach(async () => {
  await Promise.all(
    temporaryRoots.splice(0).map((root) => rm(root, { force: true, recursive: true })),
  );
});

describe("deployment artifact", () => {
  it("creates deterministic marker and manifest bytes including hidden files", async () => {
    const first = await fixture();
    const second = await fixture();

    await createDeploymentArtifact({
      sourceDirectory: first.source,
      artifactDirectory: first.artifact,
      identity,
    });
    await createDeploymentArtifact({
      sourceDirectory: second.source,
      artifactDirectory: second.artifact,
      identity,
    });

    const firstManifest = await readFile(join(first.artifact, ARTIFACT_MANIFEST));
    const secondManifest = await readFile(join(second.artifact, ARTIFACT_MANIFEST));
    expect(firstManifest).toEqual(secondManifest);

    const markerPath = join(first.artifact, ASSET_ROOT, ...DEPLOYMENT_MARKER.split("/"));
    expect(await readFile(markerPath, "utf8")).toBe(renderDeploymentMarker(identity));

    const manifest = JSON.parse(firstManifest.toString("utf8"));
    expect(manifest.markerPath).toBe(DEPLOYMENT_MARKER);
    expect(manifest.files.map((file) => file.path)).toEqual([
      ".metadata/empty",
      ".well-known/chaft-deployment.json",
      "404.html",
      "_astro/app.AbCd1234.js",
      "_headers",
      "_redirects",
      "index.html",
      "logo.bin",
    ]);
    expect(manifest.files.find((file) => file.path === "logo.bin")).toMatchObject({
      sizeBytes: 4,
      sha256: "3d1f57c984978ef98a18378c8166c1cb8ede02c03eeb6aee7e2f121dfeee3e56",
    });
  });

  it.each(mountedIdentities)(
    "stores provider files and the deployment marker at the $name physical mount",
    async ({ identity: mountedIdentity, mountPath }) => {
      const { root, source, artifact } = await fixture(mountedIdentity);
      await createDeploymentArtifact({
        sourceDirectory: source,
        artifactDirectory: artifact,
        identity: mountedIdentity,
      });

      const markerPath = deploymentMarkerPath(mountedIdentity.siteUrl);
      expect(markerPath).toBe(`${mountPath}/${DEPLOYMENT_MARKER}`);
      const manifest = await manifestAt(artifact);
      expect(manifest.markerPath).toBe(markerPath);
      expect(manifest.files.map((file) => file.path)).toEqual([
        "_headers",
        "_redirects",
        `${mountPath}/.metadata/empty`,
        markerPath,
        `${mountPath}/404.html`,
        `${mountPath}/_astro/app.AbCd1234.js`,
        `${mountPath}/index.html`,
        `${mountPath}/logo.bin`,
      ]);

      await expect(readFile(join(source, "404.html"), "utf8")).rejects.toMatchObject({
        code: "ENOENT",
      });
      await expect(
        readFile(join(artifact, ASSET_ROOT, ...DEPLOYMENT_MARKER.split("/")), "utf8"),
      ).rejects.toMatchObject({ code: "ENOENT" });
      expect(
        await readFile(join(artifact, ASSET_ROOT, ...markerPath.split("/")), "utf8"),
      ).toBe(renderDeploymentMarker(mountedIdentity));

      await expect(
        verifyDeploymentArtifact({
          artifactDirectory: artifact,
          expectedIdentity: mountedIdentity,
        }),
      ).resolves.toMatchObject({
        manifest: { markerPath },
        manifestSha256: expect.stringMatching(/^[a-f0-9]{64}$/),
      });

      const destination = join(root, `installed-${mountPath.replaceAll("/", "-")}`);
      await installDeploymentArtifact({
        artifactDirectory: artifact,
        destinationDirectory: destination,
        expectedIdentity: mountedIdentity,
      });
      expect(
        await readFile(join(destination, ...mountPath.split("/"), "index.html"), "utf8"),
      ).toBe("<h1>Chaft</h1>\n");
      expect(
        await readFile(join(destination, ...markerPath.split("/")), "utf8"),
      ).toBe(renderDeploymentMarker(mountedIdentity));
      await expect(
        compareInstalledDeploymentArtifact({
          artifactDirectory: artifact,
          destinationDirectory: destination,
          expectedIdentity: mountedIdentity,
        }),
      ).resolves.toMatchObject({
        manifest: { markerPath },
      });
    },
  );

  it("verifies and installs the exact artifact without rebuilding", async () => {
    const { root, source, artifact } = await fixture();
    await createDeploymentArtifact({ sourceDirectory: source, artifactDirectory: artifact, identity });

    const verified = await verifyDeploymentArtifact({
      artifactDirectory: artifact,
      expectedIdentity: identity,
    });
    expect(verified.manifest.files.length).toBe(8);
    expect(verified.manifestSha256).toMatch(/^[a-f0-9]{64}$/);

    const destination = join(root, "installed-dist");
    await installDeploymentArtifact({
      artifactDirectory: artifact,
      destinationDirectory: destination,
      expectedIdentity: identity,
    });
    expect(await readFile(join(destination, "index.html"), "utf8")).toBe("<h1>Chaft</h1>\n");
    expect(
      await readFile(join(destination, ...DEPLOYMENT_MARKER.split("/")), "utf8"),
    ).toBe(renderDeploymentMarker(identity));
    await expect(
      compareInstalledDeploymentArtifact({
        artifactDirectory: artifact,
        destinationDirectory: destination,
        expectedIdentity: identity,
      }),
    ).resolves.toMatchObject({
      manifestSha256: expect.stringMatching(/^[a-f0-9]{64}$/),
    });
    await writeFile(join(destination, "index.html"), "<h1>Changed</h1>\n");
    await expect(
      compareInstalledDeploymentArtifact({
        artifactDirectory: artifact,
        destinationDirectory: destination,
        expectedIdentity: identity,
      }),
    ).rejects.toThrow(/installed artifact mismatch/);

    await expect(
      installDeploymentArtifact({
        artifactDirectory: artifact,
        destinationDirectory: destination,
        expectedIdentity: identity,
      }),
    ).rejects.toThrow(/destination already exists/);
  });

  it("supports the exact machine-readable CLI contract used by workflows", async () => {
    const { root, source, artifact } = await fixture();
    const cli = join(process.cwd(), "scripts", "deployment-artifact-cli.mjs");
    const common = [
      "--repository",
      identity.sourceRepository,
      "--commit",
      identity.sourceCommit,
      "--site-url",
      identity.siteUrl,
    ];

    const created = await execFileAsync(process.execPath, [
      cli,
      "create",
      "--source",
      source,
      "--artifact",
      artifact,
      ...common,
    ]);
    expect(JSON.parse(created.stdout)).toMatchObject({
      command: "create",
      fileCount: 8,
    });

    const verified = await execFileAsync(process.execPath, [
      cli,
      "verify",
      "--",
      "--artifact",
      artifact,
      ...common,
    ]);
    expect(JSON.parse(verified.stdout)).toMatchObject({
      command: "verify",
      fileCount: 8,
      manifestSha256: expect.stringMatching(/^[a-f0-9]{64}$/),
    });

    const destination = join(root, "cli-installed-dist");
    const installed = await execFileAsync(process.execPath, [
      cli,
      "install",
      "--artifact",
      artifact,
      "--destination",
      destination,
      ...common,
    ]);
    expect(JSON.parse(installed.stdout)).toMatchObject({
      command: "install",
      destinationDirectory: destination,
      fileCount: 8,
    });

    const compared = await execFileAsync(process.execPath, [
      cli,
      "compare",
      "--artifact",
      artifact,
      "--destination",
      destination,
      ...common,
    ]);
    expect(JSON.parse(compared.stdout)).toMatchObject({
      command: "compare",
      destinationDirectory: destination,
      fileCount: 8,
    });
  });

  it("rejects modified, missing, and extra artifact files", async () => {
    for (const mutation of ["modified", "missing", "extra"]) {
      const { source, artifact } = await fixture();
      await createDeploymentArtifact({ sourceDirectory: source, artifactDirectory: artifact, identity });
      const site = join(artifact, ASSET_ROOT);

      if (mutation === "modified") {
        await writeFile(join(site, "index.html"), "<h1>Shady</h1>\n");
      } else if (mutation === "missing") {
        await unlink(join(site, "_headers"));
      } else {
        await writeFile(join(site, ".unexpected"), "extra\n");
      }

      await expect(
        verifyDeploymentArtifact({ artifactDirectory: artifact, expectedIdentity: identity }),
      ).rejects.toThrow(/artifact (?:file count differs|mismatch)/);
    }
  });

  it("rejects the validation origin and a pre-existing reserved marker", async () => {
    const validation = await fixture();
    await writeFile(
      join(validation.source, "index.html"),
      "https://website-validation.invalid/\n",
    );
    await expect(
      createDeploymentArtifact({
        sourceDirectory: validation.source,
        artifactDirectory: validation.artifact,
        identity,
      }),
    ).rejects.toThrow(/reserved validation origin/);

    const reserved = await fixture();
    const marker = join(reserved.source, ...DEPLOYMENT_MARKER.split("/"));
    await mkdir(join(reserved.source, ".well-known"));
    await writeFile(marker, "{}\n");
    await expect(
      createDeploymentArtifact({
        sourceDirectory: reserved.source,
        artifactDirectory: reserved.artifact,
        identity,
      }),
    ).rejects.toThrow(/reserved marker/);

    const mountedIdentity = mountedIdentities[1].identity;
    const mountedReserved = await fixture(mountedIdentity);
    const mountedMarker = join(
      mountedReserved.source,
      ...deploymentMarkerPath(mountedIdentity.siteUrl).split("/"),
    );
    await mkdir(join(mountedReserved.source, "products", "chaft", ".well-known"));
    await writeFile(mountedMarker, "{}\n");
    await expect(
      createDeploymentArtifact({
        sourceDirectory: mountedReserved.source,
        artifactDirectory: mountedReserved.artifact,
        identity: mountedIdentity,
      }),
    ).rejects.toThrow(/reserved marker: products\/chaft\/\.well-known/);

    const mountedRootReserved = await fixture(mountedIdentity);
    const staleRootMarker = join(
      mountedRootReserved.source,
      ...DEPLOYMENT_MARKER.split("/"),
    );
    await mkdir(dirname(staleRootMarker), { recursive: true });
    await writeFile(staleRootMarker, "{}\n");
    await expect(
      createDeploymentArtifact({
        sourceDirectory: mountedRootReserved.source,
        artifactDirectory: mountedRootReserved.artifact,
        identity: mountedIdentity,
      }),
    ).rejects.toThrow(/reserved marker: \.well-known/);
  });

  it("requires the 404 page at the identity-derived physical mount", async () => {
    const mountedIdentity = mountedIdentities[0].identity;
    const { source, artifact } = await fixture(mountedIdentity);
    await unlink(join(source, "chaft", "404.html"));

    await expect(
      createDeploymentArtifact({
        sourceDirectory: source,
        artifactDirectory: artifact,
        identity: mountedIdentity,
      }),
    ).rejects.toThrow(/missing required file: chaft\/404\.html/);
  });

  it.runIf(process.platform !== "win32")("rejects symbolic links", async () => {
    const { source, artifact } = await fixture();
    await symlink("index.html", join(source, "linked.html"));
    await expect(
      createDeploymentArtifact({ sourceDirectory: source, artifactDirectory: artifact, identity }),
    ).rejects.toThrow(/symbolic link/);
  });

  it.runIf(process.platform !== "win32")(
    "rejects output paths hidden inside the source by a symlinked parent",
    async () => {
      const { root, source } = await fixture();
      const sourceAlias = join(root, "source-alias");
      await symlink(source, sourceAlias, "dir");

      await expect(
        createDeploymentArtifact({
          sourceDirectory: source,
          artifactDirectory: join(sourceAlias, "artifact"),
          identity,
        }),
      ).rejects.toThrow(/must be a real directory|must not overlap/);
    },
  );

  it("rejects malicious, duplicate, unsorted, and unknown manifest data", async () => {
    for (const mutation of [
      "traversal",
      "duplicate",
      "unsorted",
      "marker-traversal",
      "marker-missing",
      "marker-extra",
      "unknown",
    ]) {
      const { source, artifact } = await fixture();
      await createDeploymentArtifact({ sourceDirectory: source, artifactDirectory: artifact, identity });
      const manifest = await manifestAt(artifact);

      if (mutation === "traversal") {
        manifest.files[0].path = "../outside";
      } else if (mutation === "duplicate") {
        manifest.files[1] = { ...manifest.files[0] };
      } else if (mutation === "unsorted") {
        [manifest.files[0], manifest.files[1]] = [manifest.files[1], manifest.files[0]];
      } else if (mutation === "marker-traversal") {
        manifest.markerPath = "../.well-known/chaft-deployment.json";
      } else if (mutation === "marker-missing") {
        manifest.markerPath = "other/.well-known/chaft-deployment.json";
      } else if (mutation === "marker-extra") {
        const marker = manifest.files.find((file) => file.path === DEPLOYMENT_MARKER);
        manifest.files.push({
          ...marker,
          path: `other/${DEPLOYMENT_MARKER}`,
        });
        manifest.files.sort((left, right) =>
          Buffer.compare(Buffer.from(left.path, "utf8"), Buffer.from(right.path, "utf8")),
        );
      } else {
        manifest.unreviewed = true;
      }
      await writeFile(
        join(artifact, ARTIFACT_MANIFEST),
        `${JSON.stringify(manifest, null, 2)}\n`,
      );

      await expect(
        verifyDeploymentArtifact({ artifactDirectory: artifact, expectedIdentity: identity }),
      ).rejects.toThrow();
    }
  });

  it("rejects duplicate JSON keys and invalid UTF-8 manifest bytes", async () => {
    const duplicate = await fixture();
    await createDeploymentArtifact({
      sourceDirectory: duplicate.source,
      artifactDirectory: duplicate.artifact,
      identity,
    });
    const duplicatePath = join(duplicate.artifact, ARTIFACT_MANIFEST);
    const canonical = await readFile(duplicatePath, "utf8");
    await writeFile(
      duplicatePath,
      canonical.replace(
        '  "algorithm": "sha256",',
        '  "algorithm": "sha256",\n  "algorithm": "sha256",',
      ),
    );
    await expect(
      verifyDeploymentArtifact({
        artifactDirectory: duplicate.artifact,
        expectedIdentity: identity,
      }),
    ).rejects.toThrow(/canonical generated JSON/);

    const invalidUtf8 = await fixture();
    await createDeploymentArtifact({
      sourceDirectory: invalidUtf8.source,
      artifactDirectory: invalidUtf8.artifact,
      identity,
    });
    const invalidPath = join(invalidUtf8.artifact, ARTIFACT_MANIFEST);
    const validBytes = await readFile(invalidPath);
    await writeFile(invalidPath, Buffer.concat([validBytes, Buffer.from([0xff])]));
    await expect(
      verifyDeploymentArtifact({
        artifactDirectory: invalidUtf8.artifact,
        expectedIdentity: identity,
      }),
    ).rejects.toThrow(/valid UTF-8/);
  });

  it("rejects reordered manifest properties", async () => {
    const { source, artifact } = await fixture();
    await createDeploymentArtifact({ sourceDirectory: source, artifactDirectory: artifact, identity });
    const manifestPath = join(artifact, ARTIFACT_MANIFEST);
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    const reordered = {
      artifactKind: manifest.artifactKind,
      schemaVersion: manifest.schemaVersion,
      algorithm: manifest.algorithm,
      assetRoot: manifest.assetRoot,
      markerPath: manifest.markerPath,
      files: manifest.files,
    };
    await writeFile(manifestPath, `${JSON.stringify(reordered, null, 2)}\n`);

    await expect(
      verifyDeploymentArtifact({ artifactDirectory: artifact, expectedIdentity: identity }),
    ).rejects.toThrow(/canonical generated JSON/);
  });

  it("rejects the wrong caller-owned identity", async () => {
    const { source, artifact } = await fixture();
    await createDeploymentArtifact({ sourceDirectory: source, artifactDirectory: artifact, identity });

    await expect(
      verifyDeploymentArtifact({
        artifactDirectory: artifact,
        expectedIdentity: {
          ...identity,
          sourceCommit: "f".repeat(40),
        },
      }),
    ).rejects.toThrow(/marker does not match/);
  });

  it("binds the manifest marker path to the expected site URL", async () => {
    const mountedIdentity = mountedIdentities[0].identity;
    const { source, artifact } = await fixture(mountedIdentity);
    await createDeploymentArtifact({
      sourceDirectory: source,
      artifactDirectory: artifact,
      identity: mountedIdentity,
    });

    await expect(
      verifyDeploymentArtifact({
        artifactDirectory: artifact,
        expectedIdentity: {
          ...mountedIdentity,
          siteUrl: "https://chaft.example/other",
        },
      }),
    ).rejects.toThrow(/marker path does not match the expected site URL/);
  });

  it("accepts only full SHA-1 or SHA-256 source revisions", () => {
    expect(() =>
      renderDeploymentMarker({
        ...identity,
        sourceCommit: "a".repeat(41),
      }),
    ).toThrow(/full SHA-1 or SHA-256/);
    expect(() =>
      renderDeploymentMarker({
        ...identity,
        sourceCommit: "a".repeat(64),
      }),
    ).not.toThrow();
  });

  it.each([
    "https://chaft.example/_headers/docs",
    "https://chaft.example/_REDIRECTS/docs",
    "https://chaft.example/space%20name",
    "https://chaft.example/encoded%2Fseparator",
    "https://chaft.example/a//b",
  ])("rejects a non-portable or provider-colliding site path: %s", (siteUrl) => {
    expect(() => deploymentMountPath(siteUrl)).toThrow(/pathname|provider file/);
    expect(() => renderDeploymentMarker({ ...identity, siteUrl })).toThrow(
      /pathname|provider file/,
    );
  });
});

describe("portable artifact paths", () => {
  it.each([
    "../escape",
    "/absolute",
    "C:/drive",
    "a\\windows",
    "a//empty",
    "a/./dot",
    "a/../parent",
    "trailing.",
    "NUL",
    ".env",
    "node_modules/file",
    "space name",
  ])("rejects %s", (path) => {
    expect(() => validatePortablePath(path)).toThrow();
  });
});
