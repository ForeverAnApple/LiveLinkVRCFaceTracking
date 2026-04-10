{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      pkgsFor = system: import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };

      guiDeps = pkgs: with pkgs; [
        libGL
        libxkbcommon
        wayland
        libx11
        libxcursor
        libxi
        libxrandr
      ];

      version = "0.1.0";

      mkPackage = pkgs: { pname, features ? [], extraBuildInputs ? [] }:
        pkgs.rustPlatform.buildRustPackage {
          inherit pname version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildNoDefaultFeatures = true;
          buildFeatures = features;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = extraBuildInputs;
          meta = {
            description = "Fast & lightweight LiveLink Face to VRChat OSC bridge";
            homepage = "https://github.com/ForeverAnApple/LiveLinkVRCFaceTracking";
            license = pkgs.lib.licenses.mit;
            mainProgram = "litelink";
          };
        };
    in
    {
      overlays.default = final: prev: {
        litelink = self.packages.${final.system}.default;
        litelink-gui = self.packages.${final.system}.gui;
      };

      devShells = forAllSystems (system:
        let
          pkgs = pkgsFor system;
          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" ];
          };
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              rust
              pkg-config
              cargo-nextest
            ] ++ (guiDeps pkgs);

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (guiDeps pkgs);
          };
        });

      packages = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = mkPackage pkgs {
            pname = "litelink";
          };

          gui = mkPackage pkgs {
            pname = "litelink-gui";
            features = [ "gui" ];
            extraBuildInputs = guiDeps pkgs;
          };
        });
    };
}
