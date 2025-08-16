{
  description = "Redfire Switch - High-Performance SIP Switch with BGP Anycast Clustering";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
          targets = [ "x86_64-unknown-linux-gnu" "aarch64-unknown-linux-gnu" ];
        };

        # Crane library for building Rust packages
        craneLib = crane.lib.${system}.overrideToolchain rustToolchain;

        # Common build inputs
        commonBuildInputs = with pkgs; [
          openssl
          postgresql
          rocksdb
          pkg-config
          cmake
          protobuf
          llvmPackages.clang
          llvmPackages.libclang
        ];

        # Common environment variables
        commonEnvVars = {
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.lib.getVersion pkgs.llvmPackages.clang}/include";
        };

        # Source filtering
        src = craneLib.cleanCargoSource (craneLib.path ./.);

        # Cargo artifacts for faster builds
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          buildInputs = commonBuildInputs;
          inherit (commonEnvVars) LIBCLANG_PATH BINDGEN_EXTRA_CLANG_ARGS;
        };

        # Main package
        redfire-switch = craneLib.buildPackage {
          inherit cargoArtifacts src;
          buildInputs = commonBuildInputs;
          inherit (commonEnvVars) LIBCLANG_PATH BINDGEN_EXTRA_CLANG_ARGS;

          buildFeatures = [
            "bgp-anycast"
            "redis-cluster"
            "web-ui"
          ];

          # Skip tests that require external services
          doCheck = false;

          # Install additional files
          postInstall = ''
            # Install systemd service files
            mkdir -p $out/lib/systemd/system
            cp systemd/*.service $out/lib/systemd/system/

            # Install configuration templates
            mkdir -p $out/share/redfire-switch
            find . -name "config-*.toml" -exec cp {} $out/share/redfire-switch/ \; || true
            find . -name "*-template.toml" -exec cp {} $out/share/redfire-switch/ \; || true

            # Install documentation
            mkdir -p $out/share/doc/redfire-switch
            cp README.md $out/share/doc/redfire-switch/ || true
            cp -r docs $out/share/doc/redfire-switch/ || true
            cp -r examples $out/share/doc/redfire-switch/ || true

            # Install database schema
            find . -name "*.sql" -exec cp {} $out/share/redfire-switch/ \; || true
          '';
        };

        # Docker image
        dockerImage = pkgs.dockerTools.buildLayeredImage {
          name = "redfire-switch";
          tag = "latest";
          
          contents = with pkgs; [
            redfire-switch
            cacert
            tzdata
            bash
            coreutils
          ];

          config = {
            Cmd = [ "${redfire-switch}/bin/redfire-switch" "start" ];
            ExposedPorts = {
              "5060/tcp" = {};
              "5060/udp" = {};
              "5061/tcp" = {};
              "5061/udp" = {};
              "8080/tcp" = {};
            };
            Env = [
              "RUST_LOG=info"
              "PATH=${pkgs.lib.makeBinPath [ redfire-switch pkgs.coreutils ]}"
            ];
            WorkingDir = "/var/lib/redfire-switch";
          };
        };

      in {
        # Packages
        packages = {
          default = redfire-switch;
          redfire-switch = redfire-switch;
          docker = dockerImage;
        };

        # Development shell
        devShells.default = craneLib.devShell {
          inputsFrom = [ redfire-switch ];
          
          packages = with pkgs; [
            # Development tools
            rustToolchain
            rust-analyzer
            cargo-watch
            cargo-edit
            cargo-audit
            cargo-deny
            cargo-outdated
            cargo-udeps
            
            # Database tools
            postgresql
            redis
            
            # BGP tools
            exabgp
            bird2
            
            # Network tools
            wireshark
            tcpdump
            nmap
            
            # System tools
            htop
            iotop
            strace
            
            # Build tools
            just
            bacon
          ];

          shellHook = ''
            echo "🔥 Redfire Switch Development Environment"
            echo "========================================="
            echo "Available commands:"
            echo "  cargo build --features bgp-anycast,redis-cluster"
            echo "  cargo test"
            echo "  cargo run -- --help"
            echo ""
            echo "Services (for testing):"
            echo "  PostgreSQL: pg_ctl -D /tmp/postgres init && pg_ctl -D /tmp/postgres start"
            echo "  Redis:      redis-server"
            echo ""
            echo "Documentation:"
            echo "  cargo doc --open"
            echo ""
          '';
        };

        # Apps
        apps.default = flake-utils.lib.mkApp {
          drv = redfire-switch;
          exePath = "/bin/redfire-switch";
        };

        # Checks (tests and linting)
        checks = {
          redfire-switch-clippy = craneLib.cargoClippy {
            inherit cargoArtifacts src;
            buildInputs = commonBuildInputs;
            inherit (commonEnvVars) LIBCLANG_PATH BINDGEN_EXTRA_CLANG_ARGS;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          };

          redfire-switch-fmt = craneLib.cargoFmt {
            inherit src;
          };

          redfire-switch-audit = craneLib.cargoAudit {
            inherit src;
          };

          redfire-switch-deny = craneLib.cargoDeny {
            inherit src;
          };
        };

        # Formatter
        formatter = pkgs.nixpkgs-fmt;
      }
    ) // {
      # NixOS module
      nixosModules.default = import ./nixos/module.nix;
      nixosModules.redfire-switch = import ./nixos/module.nix;

      # Overlay for adding redfire-switch to nixpkgs
      overlays.default = final: prev: {
        redfire-switch = final.callPackage ./nixos/package.nix {};
      };
    };
}