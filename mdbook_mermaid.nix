{ system, stdenv, fetchzip, }:

let
  version = "0.14.0";
  arch = if system == "aarch64-linux" then "aarch64-unknown-linux-gnu" else "x86_64-unknown-linux-gnu";
  hash = {
    "x86_64-linux" = "sha256-cbcPoLQ4b8cQ2xk0YnapC9L0Rayt0bblGXVfCzJLiGA=";
    "aarch64-linux" = throw "mdbook-mermaid: aarch64-linux hash not yet computed - run 'nix-prefetch-url --unpack <url>' to get it";
  }.${system};

in stdenv.mkDerivation {
  pname = "mdbook-mermaid";
  inherit version;

  src = fetchzip {
    url = "https://github.com/badboy/mdbook-mermaid/releases/download/v${version}/mdbook-mermaid-v${version}-${arch}.tar.gz";
    inherit hash;
  };

  installPhase = ''
    runHook preInstall
    mkdir -p $out/bin
    cp mdbook-mermaid $out/bin/
    runHook postInstall
  '';
}
