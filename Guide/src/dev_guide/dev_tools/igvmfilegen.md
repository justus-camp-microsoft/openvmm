# igvmfilegen

`igvmfilegen` is the tool that assembles IGVM files for OpenHCL. In addition
to building IGVM files from manifests, it includes a `diff` subcommand for
comparing two IGVM files side-by-side.

## `igvmfilegen diff`

The `diff` subcommand decomposes two IGVM files into their logical parts
(using the `.bin.map` file to name regions by component) and runs
[diffoscope](https://diffoscope.org/) on the extracted directory trees.

This is particularly useful for investigating reproducibility failures in CI,
where two builds of the same commit produce IGVM files that differ.

### Prerequisites

Install [diffoscope](https://diffoscope.org/):

```bash
pip install diffoscope
```

### Usage

```bash
cargo run -p igvmfilegen -- diff \
  --left <left.bin> \
  --right <right.bin> \
  --left-map <left.bin.map> \
  --right-map <right.bin.map> \
  [--keep-extracted] \
  [-- <diffoscope args...>]
```

- `--left` / `--right`: The two IGVM files to compare.
- `--left-map` / `--right-map`: The `.bin.map` files produced alongside
  each IGVM file. Used to split page data into named components
  (`underhill-kernel`, `underhill-initrd`, etc.) so diffoscope can detect
  their file formats and recurse into them.
- `--keep-extracted`: Don't delete temp dirs after diffoscope exits. Prints
  their paths to stderr for manual inspection.
- Trailing args after `--` are forwarded to diffoscope
  (e.g. `--html report.html`, `--text -`, `--max-text-report-size 0`).

### Smoke test

Identical files should produce no diff:

```bash
cargo xflowey build-igvm x64

cargo run -p igvmfilegen -- diff \
  --left flowey-out/artifacts/build-igvm/debug/x64/openhcl-x64.bin \
  --right flowey-out/artifacts/build-igvm/debug/x64/openhcl-x64.bin \
  --left-map \
    flowey-out/artifacts/build-igvm/debug/x64/openhcl-x64.bin.map \
  --right-map \
    flowey-out/artifacts/build-igvm/debug/x64/openhcl-x64.bin.map
# Expected output: "No differences found."
```

### Comparing two local builds

```bash
cargo xflowey build-igvm x64
cargo xflowey build-igvm x64-cvm

cargo run -p igvmfilegen -- diff \
  --left flowey-out/artifacts/build-igvm/debug/x64/openhcl-x64.bin \
  --right flowey-out/artifacts/build-igvm/debug/x64-cvm/openhcl-x64-cvm.bin \
  --left-map \
    flowey-out/artifacts/build-igvm/debug/x64/openhcl-x64.bin.map \
  --right-map \
    flowey-out/artifacts/build-igvm/debug/x64-cvm/openhcl-x64-cvm.bin.map \
  --keep-extracted
```

### Comparing CI artifacts

You can download IGVM artifacts from CI and compare them locally.
The IGVM binary is in the `*-openhcl-igvm` artifact and the `.bin.map`
file is in the corresponding `*-openhcl-igvm-extras` artifact.

```bash
# Download an IGVM artifact and its extras from two different runs
gh run download <run-id-a> --repo microsoft/openvmm \
  --name x64-openhcl-igvm --dir /tmp/build-a
gh run download <run-id-a> --repo microsoft/openvmm \
  --name x64-openhcl-igvm-extras --dir /tmp/extras-a
gh run download <run-id-b> --repo microsoft/openvmm \
  --name x64-openhcl-igvm --dir /tmp/build-b
gh run download <run-id-b> --repo microsoft/openvmm \
  --name x64-openhcl-igvm-extras --dir /tmp/extras-b

# Run the diff
cargo run -p igvmfilegen -- diff \
  --left /tmp/build-a/openhcl.bin \
  --right /tmp/build-b/openhcl.bin \
  --left-map /tmp/extras-a/openhcl/openhcl.bin.map \
  --right-map /tmp/extras-b/openhcl/openhcl.bin.map \
  --keep-extracted
```

### Extracted directory structure

Each IGVM file is extracted into:

```text
<tempdir>/
  headers/
    platforms.txt              # Debug-formatted platform headers
    initializations.txt        # Debug-formatted initialization headers
  regions/
    underhill-kernel_0.bin     # Raw loaded ELF segments
    underhill-initrd.cpio.gz   # Verbatim gzip cpio archive
    underhill-boot-shim_0.bin  # Raw loaded ELF segments
    sidecar-kernel_0.bin       # Raw loaded ELF segments
    ...
  regions.txt                  # GPA range, page count, flags per region
  vp_context/
    snp_vp0.bin                # SNP VMSA as raw binary
    x64_vbs_Vtl2_vp0.txt      # VBS register list as formatted text
    ...
  parameter_areas/
    area_0000.bin              # ParameterArea initial_data by index
  metadata.txt                 # Non-PageData, non-VP-context directives
```

Pages at the same GPA with different compatibility masks (SNP/TDX/VBS)
are deduplicated since the data is identical.

Components are assigned file extensions based on their content format:
the initrd gets `.cpio.gz`, command-line strings get `.txt`, device trees
get `.dtb`, and everything else gets `.bin`.
