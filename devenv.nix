{
  pkgs,
  lib,
  ...
}: let
  runtimeLibs = with pkgs; [
    libglvnd
    libxkbcommon
    mesa
    vulkan-loader
    wayland
    wayland-protocols
    libx11
    libxcursor
    libxi
    libxrandr
    libxrender
    libxext
    libxfixes
    libxcb
    libxinerama
  ];
in {
  cachix.enable = false;
  devenv.warnOnNewVersion = false;

  languages.rust = {
    enable = true;
  };

  packages = with pkgs; [
    bacon
    cargo-nextest
    cargo-watch
    just
    pkg-config
    openssl
    turso-cli
    libglvnd
    libxkbcommon
    mesa
    vulkan-loader
    wayland
    wayland-protocols
    libx11
    libxcursor
    libxi
    libxrandr
    libxrender
    libxext
    libxfixes
    libxcb
    libxinerama
  ];

  env = {
    CARGO_TERM_COLOR = "always";
    RUST_BACKTRACE = "1";
    RUST_LOG = "info";
    MAIL_APP_NAME = "mail";
  };

  scripts = {
    build.exec = "cargo build --workspace";
    check.exec = "cargo check --workspace";
    fmt.exec = "cargo fmt --all";
    lint.exec = "cargo clippy --workspace --all-targets -- -D warnings";
    test.exec = "cargo test --workspace";
    run.exec = "cargo run -p mail";
  };

  enterShell = ''
    export LD_LIBRARY_PATH="${lib.makeLibraryPath runtimeLibs}:$LD_LIBRARY_PATH"
    echo "mail devenv actief"
    echo "Gebruik: build | check | fmt | lint | test | run"
  '';
}
