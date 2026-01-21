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

  mdbook = pkgs.callPackage ./mdbook.nix { };
  mdbook_admonish = pkgs.callPackage ./mdbook_admonish.nix { };
  mdbook_mermaid = pkgs.callPackage ./mdbook_mermaid.nix { };

  protoc = pkgs.callPackage ./protoc.nix { };

  lxutil = pkgs.callPackage ./lxutil.nix { };
  openhcl_kernel = pkgs.callPackage ./openhcl_kernel.nix { };
  openvmm_deps = pkgs.callPackage ./openvmm_deps.nix { };
  uefi_mu_msvm = pkgs.callPackage ./uefi_mu_msvm.nix { };

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
    targets = [ "x86_64-unknown-linux-musl" "x86_64-unknown-none" ];
  };
in pkgs.mkShell.override { } {
  nativeBuildInputs = [
    rust
    mdbook
    mdbook_admonish
    mdbook_mermaid
    protoc
  ] ++ (with pkgs; [
    libarchive
    git
    perl
    python3
    pkg-config
  ]);
  buildInputs = [
    pkgs.openssl.dev
  ];
  CARGO_BUILD_ARGS = "--use-local-deps --custom-openvmm-deps ${openvmm_deps} --custom-uefi=${uefi_mu_msvm}/MSVM.fd --custom-kernel ${openhcl_kernel}/vmlinux --custom-kernel-modules ${openhcl_kernel}/modules --custom-protoc ${protoc}";
  OPENVMM_DEPS = openvmm_deps;
  RUST_BACKTRACE = 1;
  # will probably need more than one of these for local source + dependencies.
  # RUSTFLAGS = "--remap-path-prefix =/src";
  SOURCE_DATE_EPOCH = 12345;
  REALGCC = "gcc";
}
