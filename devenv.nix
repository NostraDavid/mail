{pkgs, ...}: {
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
    webkitgtk_4_1
    gtk3
    libsoup_3
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
    echo "mail devenv actief"
    echo "Gebruik: build | check | fmt | lint | test | run"
  '';
}
