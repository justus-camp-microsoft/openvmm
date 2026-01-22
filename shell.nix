let
  # Pinned nixpkgs for reproducibility (nixos-24.11 branch, 2025-01-21)
  nixpkgs = fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/50ab793786d9de88ee30ec4e4c24fb4236fc2674.tar.gz";
    sha256 = "1s2gr5rcyqvpr58vxdcb095mdhblij9bfzaximrva2243aal3dgx";
  };
  # Pinned rust-overlay (2025-01-21)
  rust_overlay = import (builtins.fetchTarball {
    url = "https://github.com/oxalica/rust-overlay/archive/2ef5b3362af585a83bafd34e7fc9b1f388c2e5e2.tar.gz";
    sha256 = "138a0p83qzflw8wj4a7cainqanjmvjlincx8imr3yq1b924lg9cz";
  });
  pkgs = import nixpkgs { overlays = [ rust_overlay ]; };

  # Host architecture detection
  hostArch = if pkgs.system == "aarch64-linux" then "aarch64" else "x86_64";

  # Host tools (architecture-independent)
  mdbook = pkgs.callPackage ./mdbook.nix { };
  mdbook_admonish = pkgs.callPackage ./mdbook_admonish.nix { };
  mdbook_mermaid = pkgs.callPackage ./mdbook_mermaid.nix { };
  protoc = pkgs.callPackage ./protoc.nix { };
  lxutil = pkgs.callPackage ./lxutil.nix { };

  # Helper to create deps for a specific target architecture
  mkDepsForArch = arch: {
    openvmm_deps = pkgs.callPackage ./openvmm_deps.nix { targetArch = arch; };
    openhcl_kernel = pkgs.callPackage ./openhcl_kernel.nix {
      targetArch = arch;
      is_dev = false;
      is_cvm = false;
    };
    uefi_mu_msvm = pkgs.callPackage ./uefi_mu_msvm.nix { targetArch = arch; };
  };

  # Get deps for both architectures
  x64Deps = mkDepsForArch "x86_64";
  aarch64Deps = mkDepsForArch "aarch64";

  # Cross-compilers based on host architecture
  # On x64 host: provide aarch64 cross-compiler
  # On aarch64 host: provide x64 cross-compiler
  aarch64CrossGcc = pkgs.pkgsCross.aarch64-multiplatform.buildPackages.gcc;
  x64CrossGcc = pkgs.pkgsCross.gnu64.buildPackages.gcc;

  # Native gcc (for native architecture builds)
  nativeGcc = pkgs.gcc;

  crossCompilers =
    if hostArch == "x86_64" then [ aarch64CrossGcc ]
    else [ x64CrossGcc ];

  # Rust configuration
  overrides = (builtins.fromTOML (builtins.readFile ./Cargo.toml));
  rustVersionFromCargo = overrides.workspace.package.rust-version;
  # Cargo.toml uses "X.Y", rust-overlay uses "X.Y.Z"
  # Find the latest patch version available for the given MAJOR.MINOR
  availableVersions = builtins.attrNames pkgs.rust-bin.stable;
  matchingVersions = builtins.filter
    (v: pkgs.lib.hasPrefix "${rustVersionFromCargo}." v)
    availableVersions;
  rustVersion =
    if builtins.length matchingVersions == 0
    then throw "No rust version matching ${rustVersionFromCargo}.* found in rust-overlay"
    else builtins.head (builtins.sort (a: b: a > b) matchingVersions);

  rust = pkgs.rust-bin.stable.${rustVersion}.default.override {
    extensions = [
      "rust-src" # for rust-analyzer
      "rust-analyzer"
    ];
    # Include both musl targets for cross-compilation
    targets = [
      "x86_64-unknown-linux-musl"
      "x86_64-unknown-none"
      "aarch64-unknown-linux-musl"
      "aarch64-unknown-none"
    ];
  };

  # Build CARGO_BUILD_ARGS for specific architecture
  # x86_64 uses vmlinux, aarch64 uses Image
  mkCargoBuildArgs = arch: deps:
    let kernelFile = if arch == "x86_64" then "vmlinux" else "Image";
    in "--use-local-deps --custom-openvmm-deps ${deps.openvmm_deps} --custom-uefi=${deps.uefi_mu_msvm}/MSVM.fd --custom-kernel ${deps.openhcl_kernel}/${kernelFile} --custom-kernel-modules ${deps.openhcl_kernel}/modules --custom-protoc ${protoc}";

in pkgs.mkShell {
  nativeBuildInputs = [
    rust
    mdbook
    mdbook_admonish
    mdbook_mermaid
    protoc
    nativeGcc
  ] ++ crossCompilers ++ (with pkgs; [
    libarchive
    git
    perl
    python3
    pkg-config
    binutils
  ]);

  buildInputs = [
    pkgs.openssl.dev
  ];

  # Sysroot paths for linker wrappers (used by build_support/underhill_cross/*-underhill-musl-gcc)
  X86_64_SYSROOT = "${x64Deps.openvmm_deps}";
  AARCH64_SYSROOT = "${aarch64Deps.openvmm_deps}";

  # Architecture-specific build args
  CARGO_BUILD_ARGS_X64 = mkCargoBuildArgs "x86_64" x64Deps;
  CARGO_BUILD_ARGS_AARCH64 = mkCargoBuildArgs "aarch64" aarch64Deps;

  # Default CARGO_BUILD_ARGS for native builds (based on host architecture)
  CARGO_BUILD_ARGS =
    if hostArch == "x86_64"
    then mkCargoBuildArgs "x86_64" x64Deps
    else mkCargoBuildArgs "aarch64" aarch64Deps;

  # Expose deps for reference
  OPENVMM_DEPS_X64 = x64Deps.openvmm_deps;
  OPENVMM_DEPS_AARCH64 = aarch64Deps.openvmm_deps;

  RUST_BACKTRACE = 1;
  SOURCE_DATE_EPOCH = 12345;
  # Don't set REALGCC - let the linker wrappers use their defaults

  shellHook = ''
    # Create a temp bin directory with symlinks using the expected gcc names
    # The linker wrappers expect aarch64-linux-gnu-gcc and x86_64-linux-gnu-gcc
    # but nixpkgs provides aarch64-unknown-linux-gnu-gcc and x86_64-unknown-linux-gnu-gcc (for cross)
    # and just gcc (for native)
    export NIX_CC_WRAPPER_DIR=$(mktemp -d)
    ${if hostArch == "x86_64" then ''
    # On x64 host:
    # - Native x64 gcc symlinks (for native builds)
    ln -sf ${nativeGcc}/bin/gcc $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-gcc
    ln -sf ${nativeGcc}/bin/g++ $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-g++
    ln -sf ${nativeGcc}/bin/ld $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-ld
    ln -sf ${nativeGcc}/bin/ar $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-ar
    ln -sf ${pkgs.binutils}/bin/objcopy $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-objcopy
    # - Aarch64 cross-compiler symlinks (for cross builds)
    ln -sf ${aarch64CrossGcc}/bin/aarch64-unknown-linux-gnu-gcc $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-gcc
    ln -sf ${aarch64CrossGcc}/bin/aarch64-unknown-linux-gnu-g++ $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-g++
    ln -sf ${aarch64CrossGcc}/bin/aarch64-unknown-linux-gnu-ld $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-ld
    ln -sf ${aarch64CrossGcc}/bin/aarch64-unknown-linux-gnu-ar $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-ar
    ln -sf ${aarch64CrossGcc}/bin/aarch64-unknown-linux-gnu-objcopy $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-objcopy
    '' else ''
    # On aarch64 host:
    # - Native aarch64 gcc symlinks (for native builds)
    ln -sf ${nativeGcc}/bin/gcc $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-gcc
    ln -sf ${nativeGcc}/bin/g++ $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-g++
    ln -sf ${nativeGcc}/bin/ld $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-ld
    ln -sf ${nativeGcc}/bin/ar $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-ar
    ln -sf ${pkgs.binutils}/bin/objcopy $NIX_CC_WRAPPER_DIR/aarch64-linux-gnu-objcopy
    # - x64 cross-compiler symlinks (for cross builds)
    ln -sf ${x64CrossGcc}/bin/x86_64-unknown-linux-gnu-gcc $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-gcc
    ln -sf ${x64CrossGcc}/bin/x86_64-unknown-linux-gnu-g++ $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-g++
    ln -sf ${x64CrossGcc}/bin/x86_64-unknown-linux-gnu-ld $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-ld
    ln -sf ${x64CrossGcc}/bin/x86_64-unknown-linux-gnu-ar $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-ar
    ln -sf ${x64CrossGcc}/bin/x86_64-unknown-linux-gnu-objcopy $NIX_CC_WRAPPER_DIR/x86_64-linux-gnu-objcopy
    ''}
    export PATH="$NIX_CC_WRAPPER_DIR:$PATH"

    echo "OpenVMM Nix Shell"
    echo "================="
    echo "Host architecture: ${hostArch}"
    echo ""
    echo "Sysroots available:"
    echo "  X86_64_SYSROOT=$X86_64_SYSROOT"
    echo "  AARCH64_SYSROOT=$AARCH64_SYSROOT"
    echo ""
    echo "Build commands:"
    echo "  cargo xflowey build-igvm x64 \$CARGO_BUILD_ARGS_X64"
    echo "  cargo xflowey build-igvm aarch64 \$CARGO_BUILD_ARGS_AARCH64"
    echo ""
  '';
}
