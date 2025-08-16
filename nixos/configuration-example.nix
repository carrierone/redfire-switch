# Example NixOS configuration for Redfire Switch
{ config, pkgs, ... }:

{
  imports = [
    # Import the Redfire Switch module
    (builtins.fetchGit {
      url = "https://github.com/carrierone/redfire-switch.git";
      ref = "main";
    } + "/nixos/module.nix")
  ];

  # Basic Redfire Switch configuration
  services.redfire-switch = {
    enable = true;
    openFirewall = true;
    
    # Main configuration
    settings = {
      sip = {
        bind_address = "0.0.0.0:5060";
        external_ip = "203.0.113.100"; # Your public IP
        domain = "sip.example.com";
        transport = "UDP";
        tls = {
          enabled = true;
          bind_address = "0.0.0.0:5061";
          certificate_path = "/var/lib/acme/sip.example.com/cert.pem";
          private_key_path = "/var/lib/acme/sip.example.com/key.pem";
        };
      };

      # Database configuration
      database = {
        url = "postgresql://redfire:secure_password@localhost/redfire_switch";
        pool_size = 20;
        timeout = 30;
      };

      # Routing configuration
      routing = {
        default_route = "carrier1";
        lcr_enabled = true;
        quality_routing = true;
        failover_enabled = true;
      };

      # Security and fraud protection
      security = {
        rate_limiting = {
          enabled = true;
          max_calls_per_second = 100;
          max_registrations_per_minute = 60;
        };
        fraud_detection = {
          enabled = true;
          threshold_score = 75;
          block_suspicious_calls = true;
        };
        stir_shaken = {
          enabled = true;
          verification_required = false;
          certificate_path = "/etc/ssl/certs/stir-shaken.pem";
        };
      };

      # Monitoring and logging
      monitoring = {
        metrics_enabled = true;
        prometheus_endpoint = "127.0.0.1:9090";
        health_check_interval = 30;
      };

      logging = {
        level = "info";
        format = "json";
        output = "journald";
      };
    };

    # BGP Anycast clustering
    bgpAnycast = {
      enable = true;
      settings = {
        enabled = true;
        node = {
          node_id = "sip-switch-01";
          name = "Primary SIP Switch";
          region = "us-east-1";
          zone = "us-east-1a";
          priority = 100;
          capacity = 10000;
        };

        bgp = {
          daemon = "ExaBgp";
          router_id = "203.0.113.100";
          local_as = 65001;
          med = 100;
          
          neighbors = [
            {
              address = "203.0.113.1";
              remote_as = 65000;
              password = "bgp_neighbor_password";
            }
            {
              address = "203.0.113.2"; 
              remote_as = 65000;
              password = "bgp_neighbor_password";
            }
          ];

          # IP addresses to advertise via BGP
          advertised_prefixes = [
            {
              prefix = "203.0.113.100/32";
              next_hop = null;
              communities = [ 65001 ];
            }
          ];
        };

        # Session storage for clustering
        session_store = {
          store_type = "Redis";
          connection = {
            urls = [ "redis://127.0.0.1:6379/0" ];
            pool_size = 10;
            timeout = 5000;
          };
          compression = "Lz4";
          ttl = 3600;
        };

        # Cluster membership
        cluster = {
          protocol = "Gossip";
          gossip = {
            bind_addr = "0.0.0.0:7946";
            seeds = [
              "203.0.113.101:7946"
              "203.0.113.102:7946"
            ];
            interval = 1000;
            node_timeout = 10000;
            suspicion_timeout = 5000;
          };
        };

        # Health monitoring
        health = {
          enabled = true;
          check_interval = 30000;
          check_timeout = 5000;
          failure_threshold = 3;
          recovery_threshold = 2;
        };
      };
    };

    # Web management interface
    webInterface = {
      enable = true;
      bind = "0.0.0.0:8080";
    };

    # Database setup
    database = {
      createLocally = true;
      name = "redfire_switch";
      user = "redfire";
    };
  };

  # Additional services and configuration

  # PostgreSQL optimization for SIP workloads
  services.postgresql = {
    enable = true;
    package = pkgs.postgresql_15;
    settings = {
      shared_buffers = "256MB";
      effective_cache_size = "1GB";
      maintenance_work_mem = "64MB";
      checkpoint_completion_target = 0.9;
      wal_buffers = "16MB";
      default_statistics_target = 100;
      random_page_cost = 1.1;
      effective_io_concurrency = 200;
      work_mem = "4MB";
      min_wal_size = "1GB";
      max_wal_size = "4GB";
      max_connections = 200;
    };
  };

  # Redis configuration for session storage
  services.redis.servers.redfire-switch = {
    enable = true;
    bind = "127.0.0.1";
    port = 6379;
    settings = {
      maxmemory = "512mb";
      maxmemory-policy = "allkeys-lru";
      save = "900 1 300 10 60 10000";
      tcp-keepalive = 300;
    };
  };

  # Nginx reverse proxy for web interface (optional)
  services.nginx = {
    enable = true;
    recommendedTlsSettings = true;
    recommendedOptimisation = true;
    recommendedGzipSettings = true;
    
    virtualHosts."admin.sip.example.com" = {
      enableACME = true;
      forceSSL = true;
      locations."/" = {
        proxyPass = "http://127.0.0.1:8080";
        proxyWebsockets = true;
        extraConfig = ''
          proxy_set_header Host $host;
          proxy_set_header X-Real-IP $remote_addr;
          proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
          proxy_set_header X-Forwarded-Proto $scheme;
        '';
      };
    };
  };

  # Let's Encrypt certificates
  security.acme = {
    acceptTerms = true;
    defaults.email = "admin@example.com";
    certs."sip.example.com" = {
      domain = "sip.example.com";
      extraDomainNames = [ "admin.sip.example.com" ];
      group = "redfire";
    };
  };

  # Firewall configuration
  networking.firewall = {
    allowedTCPPorts = [ 
      22    # SSH
      80    # HTTP
      443   # HTTPS
      5060  # SIP
      5061  # SIP TLS
    ];
    allowedUDPPorts = [
      5060  # SIP
      5061  # SIP TLS
      7946  # Cluster gossip
    ];
    allowedUDPPortRanges = [
      { from = 10000; to = 20000; } # RTP media
    ];
  };

  # System optimization for SIP workloads
  boot.kernel.sysctl = {
    # Network optimizations
    "net.core.rmem_max" = 134217728;
    "net.core.wmem_max" = 134217728;
    "net.ipv4.tcp_rmem" = "4096 87380 134217728";
    "net.ipv4.tcp_wmem" = "4096 65536 134217728";
    "net.core.netdev_max_backlog" = 5000;
    "net.ipv4.tcp_congestion_control" = "bbr";
    
    # File descriptor limits
    "fs.file-max" = 1048576;
    
    # Memory management
    "vm.swappiness" = 10;
    "vm.dirty_ratio" = 15;
    "vm.dirty_background_ratio" = 5;
  };

  # Monitoring stack (optional)
  services.prometheus = {
    enable = true;
    port = 9090;
    
    scrapeConfigs = [
      {
        job_name = "redfire-switch";
        static_configs = [{
          targets = [ "127.0.0.1:9091" ]; # Redfire Switch metrics endpoint
        }];
      }
    ];
  };

  services.grafana = {
    enable = true;
    settings.server = {
      http_addr = "127.0.0.1";
      http_port = 3000;
    };
  };

  # Log aggregation
  services.loki = {
    enable = true;
    configuration = {
      server.http_listen_port = 3100;
      auth_enabled = false;
      
      ingester = {
        lifecycler = {
          address = "127.0.0.1";
          ring = {
            kvstore.store = "inmemory";
            replication_factor = 1;
          };
        };
        chunk_idle_period = "1h";
        max_chunk_age = "1h";
        chunk_target_size = 1048576;
        chunk_retain_period = "30s";
        max_transfer_retries = 0;
      };
      
      schema_config.configs = [{
        from = "2023-01-01";
        store = "boltdb-shipper";
        object_store = "filesystem";
        schema = "v11";
        index.prefix = "index_";
        index.period = "24h";
      }];
      
      storage_config = {
        boltdb_shipper = {
          active_index_directory = "/var/lib/loki/boltdb-shipper-active";
          cache_location = "/var/lib/loki/boltdb-shipper-cache";
          cache_ttl = "24h";
          shared_store = "filesystem";
        };
        filesystem.directory = "/var/lib/loki/chunks";
      };
      
      limits_config = {
        reject_old_samples = true;
        reject_old_samples_max_age = "168h";
      };
      
      chunk_store_config.max_look_back_period = "0s";
      
      table_manager = {
        retention_deletes_enabled = false;
        retention_period = "0s";
      };
      
      compactor = {
        working_directory = "/var/lib/loki";
        shared_store = "filesystem";
      };
    };
  };

  # Security hardening
  security.sudo.wheelNeedsPassword = true;
  services.openssh = {
    enable = true;
    settings = {
      PasswordAuthentication = false;
      PermitRootLogin = "no";
    };
  };

  # Fail2ban for additional security
  services.fail2ban = {
    enable = true;
    jails.sshd.settings = {
      enabled = true;
      port = "ssh";
    };
  };

  # Automatic updates
  system.autoUpgrade = {
    enable = true;
    dates = "04:00";
    allowReboot = false;
  };

  # Backup configuration (example using restic)
  services.restic.backups.redfire = {
    initialize = true;
    repository = "s3:s3.amazonaws.com/your-backup-bucket/redfire-switch";
    passwordFile = "/etc/nixos/restic-password";
    
    paths = [
      "/var/lib/redfire-switch"
      "/etc/redfire-switch"
      "/var/lib/postgresql"
    ];
    
    timerConfig = {
      OnCalendar = "daily";
      Persistent = true;
    };
    
    extraBackupArgs = [
      "--exclude-caches"
      "--one-file-system"
    ];
  };

  # Package management
  nixpkgs.config.allowUnfree = true;
  environment.systemPackages = with pkgs; [
    htop
    iotop
    tcpdump
    wireshark
    nmap
    curl
    wget
    git
    vim
    tmux
  ];
}