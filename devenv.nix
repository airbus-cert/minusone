{pkgs, ...}: {
  packages = [
    pkgs.maturin
    pkgs.wget
    pkgs.nil
    pkgs.nixd
    pkgs.alejandra
    pkgs.just
    pkgs.cargo-component
    pkgs.just
    pkgs.gnumake
  ];

  languages = {
    python = {
      enable = true;
      package = pkgs.python3.withPackages (ps: [ ps.pip ]);
    };
    javascript = {
      enable = true;
      npm.enable = true;
    };
    rust = {
      channel = "nightly";
      components = [
        "cargo"
        "rust-src"
        "rustc"
        "rust-analyzer"
        "clippy"
        "miri"
      ];
      targets = [
        "wasm32-wasip1"
        "wasm32-wasip2"
        "x86_64-unknown-linux-gnu"
      ];
      enable = true;
    };
  };

  enterTest = ''
    just test
    just test-py
  '';
}
