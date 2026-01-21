{ system, stdenv, fetchzip, }:

let
  version = "1.18.0";
  arch = if system == "aarch64-linux" then "aarch64-unknown-linux-gnu" else "x86_64-unknown-linux-gnu";
  hash = {
    "x86_64-linux" = "sha256-L7Vt3a1vz1aO4ItCSpKqn+413JGZZ9R+ukqgsE38fMc=";
    "aarch64-linux" = throw "mdbook-admonish: aarch64-linux hash not yet computed - run 'nix-prefetch-url --unpack <url>' to get it";
  }.${system};

in stdenv.mkDerivation {
  pname = "mdbook-admonish";
  inherit version;

  src = fetchzip {
    url = "https://github.com/tommilligan/mdbook-admonish/releases/download/v${version}/mdbook-admonish-v${version}-${arch}.tar.gz";
    inherit hash;
  };

  installPhase = ''
    runHook preInstall
    mkdir -p $out/bin
    cp mdbook-admonish $out/bin/
    runHook postInstall
  '';
}
