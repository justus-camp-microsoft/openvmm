{ system, stdenv, fetchzip, targetArch ? null }:

let
  # Allow explicit override of architecture, otherwise derive from host system
  # Note: targetArch uses "x86_64"/"aarch64", but URLs use "x64"/"AARCH64"
  arch = if targetArch == "x86_64" then "x64"
         else if targetArch == "aarch64" then "AARCH64"
         else if system == "aarch64-linux" then "AARCH64"
         else "x64";
  hash = {
    "AARCH64" = "sha256-WFMMf9LdCd0X6jwPVhYScmoXjpQdJnswFwHjMWvmZz8=";
    "x64" = "sha256-wJeRZC6sd+tNSYHdyyN4Qj/sn5puT6R8eagFlHa6pP4=";
  }.${arch};

in stdenv.mkDerivation {
  pname = "uefi-mu-msvm-${arch}";
  version = "24.0.4";

  src = fetchzip {
    url =
      "https://github.com/microsoft/mu_msvm/releases/download/v24.0.4/RELEASE-${arch}-artifacts.zip";
    stripRoot = false;
    inherit hash;
  };

  installPhase = ''
    runHook preInstall
    mkdir $out
    cp FV/MSVM.fd $out
    runHook postInstall
  '';
}
