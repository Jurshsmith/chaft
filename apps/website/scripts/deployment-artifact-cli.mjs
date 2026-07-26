import {
  compareInstalledDeploymentArtifact,
  createDeploymentArtifact,
  installDeploymentArtifact,
  verifyDeploymentArtifact,
} from "./deployment-artifact.mjs";

function usage() {
  return `Usage:
  deployment-artifact-cli.mjs create \\
    --source <dist> --artifact <output> \\
    --repository <owner/repository> --commit <full-sha> --site-url <https-url>

  deployment-artifact-cli.mjs verify \\
    --artifact <directory> \\
    --repository <owner/repository> --commit <full-sha> --site-url <https-url>

  deployment-artifact-cli.mjs install \\
    --artifact <directory> --destination <dist> \\
    --repository <owner/repository> --commit <full-sha> --site-url <https-url>

  deployment-artifact-cli.mjs compare \\
    --artifact <directory> --destination <dist> \\
    --repository <owner/repository> --commit <full-sha> --site-url <https-url>`;
}

function parseOptions(values) {
  const options = new Map();
  for (let index = 0; index < values.length; index += 2) {
    const key = values[index];
    const value = values[index + 1];
    if (!key?.startsWith("--") || !value || value.startsWith("--")) {
      throw new Error(`invalid option sequence near ${key ?? "<end>"}\n${usage()}`);
    }
    const name = key.slice(2);
    if (options.has(name)) throw new Error(`duplicate option: ${key}`);
    options.set(name, value);
  }
  return options;
}

function takeExactOptions(options, names) {
  const expected = new Set(names);
  for (const name of options.keys()) {
    if (!expected.has(name)) throw new Error(`unknown option: --${name}`);
  }

  const result = {};
  for (const name of names) {
    const value = options.get(name);
    if (!value) throw new Error(`missing required option: --${name}`);
    result[name] = value;
  }
  return result;
}

function identity(options) {
  return {
    sourceRepository: options.repository,
    sourceCommit: options.commit,
    siteUrl: options["site-url"],
  };
}

async function main(argv) {
  const [command, ...rawOptions] = argv;
  const options = parseOptions(rawOptions[0] === "--" ? rawOptions.slice(1) : rawOptions);

  if (command === "create") {
    const values = takeExactOptions(options, [
      "source",
      "artifact",
      "repository",
      "commit",
      "site-url",
    ]);
    const manifest = await createDeploymentArtifact({
      sourceDirectory: values.source,
      artifactDirectory: values.artifact,
      identity: identity(values),
    });
    return {
      command,
      artifactDirectory: values.artifact,
      fileCount: manifest.files.length,
    };
  }

  if (command === "verify") {
    const values = takeExactOptions(options, [
      "artifact",
      "repository",
      "commit",
      "site-url",
    ]);
    const verified = await verifyDeploymentArtifact({
      artifactDirectory: values.artifact,
      expectedIdentity: identity(values),
    });
    return {
      command,
      artifactDirectory: values.artifact,
      fileCount: verified.manifest.files.length,
      manifestSha256: verified.manifestSha256,
    };
  }

  if (command === "install") {
    const values = takeExactOptions(options, [
      "artifact",
      "destination",
      "repository",
      "commit",
      "site-url",
    ]);
    const verified = await installDeploymentArtifact({
      artifactDirectory: values.artifact,
      destinationDirectory: values.destination,
      expectedIdentity: identity(values),
    });
    return {
      command,
      artifactDirectory: values.artifact,
      destinationDirectory: values.destination,
      fileCount: verified.manifest.files.length,
      manifestSha256: verified.manifestSha256,
    };
  }

  if (command === "compare") {
    const values = takeExactOptions(options, [
      "artifact",
      "destination",
      "repository",
      "commit",
      "site-url",
    ]);
    const verified = await compareInstalledDeploymentArtifact({
      artifactDirectory: values.artifact,
      destinationDirectory: values.destination,
      expectedIdentity: identity(values),
    });
    return {
      command,
      artifactDirectory: values.artifact,
      destinationDirectory: values.destination,
      fileCount: verified.manifest.files.length,
      manifestSha256: verified.manifestSha256,
    };
  }

  throw new Error(`unknown command: ${command ?? "<missing>"}\n${usage()}`);
}

try {
  const result = await main(process.argv.slice(2));
  process.stdout.write(`${JSON.stringify(result)}\n`);
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
