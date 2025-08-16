{ lib
, stdenv
, fetchFromGitHub
, rustPlatform
, pkg-config
, openssl
, postgresql
, libclang
, llvmPackages
, cmake
, protobuf
, rocksdb
, redis
, exabgp
, bird2
, SystemConfiguration
, Security
, CoreFoundation
, libiconv
}:

rustPlatform.buildRustPackage rec {
  pname = "redfire-switch";
  version = "0.1.0";

  src = fetchFromGitHub {
    owner = "carrierone";
    repo = "redfire-switch";
    rev = "v${version}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; # TODO: Update hash
  };

  cargoHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; # TODO: Update hash

  nativeBuildInputs = [
    pkg-config
    cmake
    protobuf
    llvmPackages.clang
    llvmPackages.libclang
  ];

  buildInputs = [
    openssl
    postgresql
    rocksdb
  ] ++ lib.optionals stdenv.isDarwin [
    SystemConfiguration
    Security
    CoreFoundation
    libiconv
  ];

  # Environment variables for build
  LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";
  BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${llvmPackages.libclang.lib}/lib/clang/${lib.getVersion llvmPackages.clang}/include";
  
  # Build features
  buildFeatures = [
    "bgp-anycast"
    "redis-cluster"
    "web-ui"
  ];

  # Skip tests that require network access or external services
  checkFlags = [
    "--skip=test_integration"
    "--skip=test_bgp_integration"
    "--skip=test_database_integration"
  ];

  # Don't run tests by default (many require external services)
  doCheck = false;

  # Install additional files
  postInstall = ''
    # Install systemd service files
    mkdir -p $out/lib/systemd/system
    cp systemd/*.service $out/lib/systemd/system/

    # Install configuration templates
    mkdir -p $out/share/redfire-switch
    cp -r config-templates/* $out/share/redfire-switch/ || true
    cp config-*.toml $out/share/redfire-switch/ || true

    # Install documentation
    mkdir -p $out/share/doc/redfire-switch
    cp README.md $out/share/doc/redfire-switch/
    cp -r docs/* $out/share/doc/redfire-switch/ || true

    # Install examples
    mkdir -p $out/share/doc/redfire-switch/examples
    cp -r examples/* $out/share/doc/redfire-switch/examples/ || true

    # Install database schema
    if [ -f schema.sql ]; then
      cp schema.sql $out/share/redfire-switch/
    fi

    # Install logrotate configuration
    mkdir -p $out/share/redfire-switch
    cat > $out/share/redfire-switch/logrotate << 'EOF'
    /var/log/redfire-switch/*.log {
        daily
        rotate 30
        compress
        delaycompress
        missingok
        notifempty
        create 644 redfire redfire
        postrotate
            systemctl reload redfire-switch.service
        endscript
    }
    EOF

    # Install bash completion
    mkdir -p $out/share/bash-completion/completions
    $out/bin/redfire-switch completion bash > $out/share/bash-completion/completions/redfire-switch || true

    # Install zsh completion
    mkdir -p $out/share/zsh/site-functions
    $out/bin/redfire-switch completion zsh > $out/share/zsh/site-functions/_redfire-switch || true

    # Install fish completion
    mkdir -p $out/share/fish/vendor_completions.d
    $out/bin/redfire-switch completion fish > $out/share/fish/vendor_completions.d/redfire-switch.fish || true
  '';

  meta = with lib; {
    description = "High-Performance SIP Switch with BGP Anycast Clustering";
    longDescription = ''
      Redfire Switch is a carrier-grade SIP switch implementation written in Rust
      that provides high-performance call routing, billing, and fraud detection.

      Features include:
      * High-performance SIP stack with RFC 3261 compliance
      * Advanced routing with LCR, quality metrics, and failover
      * Real-time billing engine with CDR generation
      * BGP Anycast clustering for geographic distribution
      * STIR/SHAKEN fraud detection and call attestation
      * Comprehensive fraud protection and rate limiting
      * Web-based management interface
      * Enterprise-grade security and monitoring
    '';
    homepage = "https://github.com/carrierone/redfire-switch";
    license = licenses.gpl3Plus;
    maintainers = with maintainers; [ /* TODO: add maintainer */ ];
    platforms = platforms.linux ++ platforms.darwin;
    mainProgram = "redfire-switch";
  };

  # Additional package outputs
  outputs = [ "out" "dev" "doc" ];

  # Development shell dependencies
  passthru.devShell = {
    buildInputs = buildInputs ++ [
      redis
      postgresql
      exabgp
      bird2
    ];
  };
}