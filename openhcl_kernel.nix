{ system, stdenv, fetchzip, targetArch ? null, is_dev ? false, is_cvm ? false }:

let
  version = if is_dev then "6.12.44.1" else "6.12.44.1";
  # Allow explicit override of architecture, otherwise derive from host system
  # Note: targetArch uses "x86_64"/"aarch64", but URLs use "x64"/"arm64"
  arch = if targetArch == "x86_64" then "x64"
         else if targetArch == "aarch64" then "arm64"
         else if system == "aarch64-linux" then "arm64"
         else "x64";
  branch = if is_dev then "hcl-dev" else "hcl-main";
  build_type = if is_cvm then "cvm" else "std";
  # See https://github.com/microsoft/OHCL-Linux-Kernel/releases
  url =
    "https://github.com/microsoft/OHCL-Linux-Kernel/releases/download/rolling-lts/${branch}/${version}/Microsoft.OHCL.Kernel${
      if is_dev then ".Dev" else ""
    }.${version}-${if is_cvm then "cvm-" else ""}${arch}.tar.gz";
  hashes = {
    hcl-main = {
      std = {
        x64 = "sha256-An1N76i1MPb+rrQ1nBpoiuxnNeD0E+VuwqXdkPzaZn0=";
        arm64 = "sha256-ENjd+Pd9sQ/f0Gvyq0iB+IG7c4p+AxwxoWu87pZSXYQ=";
      };
      cvm = {
        x64 = "sha256-pV/20epW9LYWzwA633MYxtaUCyMaLAWaaSEJyx+rniQ=";
        arm64 = throw "openhcl-kernel: cvm arm64 variant not available";
      };
    };
    hcl-dev = {
      std = {
        x64 = "sha256-Ow9piuc2IDR4RPISKY5EAQ5ykjitem4CXS9974lvwPE=";
        arm64 = throw "openhcl-kernel: dev arm64 variant not available";
      };
      cvm = {
        x64 = "sha256-IryjvoFDSghhVucKlIG9V0IzcVuf8m8Cmk5NhhWzTQM=";
        arm64 = throw "openhcl-kernel: dev cvm arm64 variant not available";
      };
    };
  };
  hash = hashes.${branch}.${build_type}.${arch};

in stdenv.mkDerivation {
  pname = "openhcl-kernel-${arch}";
  inherit version;
  src = fetchzip {
    inherit url;
    stripRoot = false;
    inherit hash;
  };

  installPhase = ''
    runHook preInstall
    mkdir -p $out/modules
    # x64 uses vmlinux, arm64 uses Image
    if [ -f vmlinux ]; then
      cp vmlinux* $out/
    fi
    if [ -f Image ]; then
      cp Image $out/
    fi
    cp -r modules/* $out/modules/
    cp kernel_build_metadata.json $out/
    runHook postInstall
  '';
}
