with import <nixpkgs> {};
stdenv.mkDerivation {
  name = "nephrite4-env";

  nativeBuildInputs = [ pkgconfig ];

  buildInputs = [
    fuse
    mecab
  ];
}
