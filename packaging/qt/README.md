# Qt package compliance assets

Every Chaft desktop package contains the files in this directory together with
Chaft's root `LICENSE`. They document the dynamically linked Qt distribution
without changing Chaft's AGPL-3.0-or-later license.

`QT-CORRESPONDING-SOURCE.json` is the machine-readable source record. It lists
only the Qt modules used by the package build: four modules on every platform,
plus `qtwayland` on Linux. Its patch order must match the source-build recipe.
The platform lists are intentional; macOS and Windows packages do not contain
Qt Wayland.

When the Qt version, module set, or patch set changes:

1. update the source URLs and SHA-256 digests;
2. copy the unmodified GPL and LGPL texts from that exact Qt source release;
3. keep the CMake install destinations and all package smoke checks green; and
4. retain the exact source inputs for as long as the associated binaries are
   distributed.

The official URLs are retrieval locations, not a transfer of Chaft release
maintainers' source-availability responsibilities. For public releases,
maintainers should retain or mirror the exact verified inputs so they remain
available with the binaries.

This file is operational guidance, not legal advice.
