# Unpublished Homebrew formula

`Formula/chaft.rb` is the unresolved review template for an optional
source-building tap. It is deliberately not publishable while its release
version, immutable tag, and full source commit remain unresolved. Keep that
template unresolved; render a separate candidate formula after the release
decision.

The template and renderer contracts can be checked at any time:

```sh
python3 packaging/homebrew/formula-contract-test.py
```

Before creating a tap:

1. Complete and verify the guided source workflow on clean Intel and Apple
   Silicon Macs.
2. Select the release version and exact release commit, create the immutable
   release tag through the reviewed release process, and verify that tag.
3. Render a candidate outside this directory, then validate its exact inputs:

   ```sh
   version=X.Y.Z
   tag="v${version}"
   commit="$(git rev-parse --verify "${tag}^{commit}")"
   staging_dir="$(mktemp -d)"
   resolved_formula="${staging_dir}/Formula/chaft.rb"

   python3 packaging/homebrew/render-formula.py \
     --version "$version" \
     --tag "$tag" \
     --commit "$commit" \
     --output "$resolved_formula"
   python3 packaging/homebrew/formula-contract-test.py \
     --resolved-formula "$resolved_formula" \
     --version "$version" \
     --tag "$tag" \
     --commit "$commit"
   ```

   The resolved validator rejects placeholders, moving branches, mismatched
   version/tag/commit coordinates, and a source workflow that does not build
   the committed Cargo dependency graph with `--locked`.
4. Run strict Homebrew audit plus clean native formula installation and test
   jobs on both architectures using that resolved candidate.
5. Decide which GitHub account owns the tap, whether its repository will be
   `homebrew-chaft`, and who may publish or update the formula.
6. Only after that decision, place the validated candidate in the reviewed tap
   repository and publish it as a separate external release action.

If the selected account is `Jurshsmith` and the repository is
`homebrew-chaft`, the eventual user experience is:

```sh
brew tap jurshsmith/chaft
brew install --build-from-source chaft
```

The rendered formula clones the fixed Git revision and invokes
`tools/macos/build-local.sh`; it does not download or repackage an official
binary. The formula binds that workflow to the exact Homebrew executable that
started the build. With `--no-install-deps`, the workflow only inspects the
already declared Homebrew dependencies and fails instead of starting a nested
package installation. The resulting app is locally ad-hoc signed, not
Developer ID signed or notarized, and must not be redistributed as a trusted
binary.

Publishing the tap repository, creating its release coordinates, and pushing
the release tag are separate external release actions. This directory performs
none of them.
