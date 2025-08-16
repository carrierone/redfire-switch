{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.redfire-switch;
  
  # Configuration file generation
  configFile = pkgs.writeText "config.toml" (generators.toTOML {} cfg.settings);
  
  bgpConfigFile = pkgs.writeText "bgp-anycast.toml" (generators.toTOML {} cfg.bgpAnycast);
  
  # Redfire Switch package
  redfire-switch = pkgs.callPackage ./package.nix {};

in {
  
  ###### Interface
  
  options.services.redfire-switch = {
    enable = mkEnableOption "Redfire Switch SIP server";
    
    package = mkOption {
      type = types.package;
      default = redfire-switch;
      defaultText = literalExpression "pkgs.redfire-switch";
      description = "The Redfire Switch package to use.";
    };
    
    user = mkOption {
      type = types.str;
      default = "redfire";
      description = "User under which Redfire Switch runs.";
    };
    
    group = mkOption {
      type = types.str;
      default = "redfire";
      description = "Group under which Redfire Switch runs.";
    };
    
    dataDir = mkOption {
      type = types.path;
      default = "/var/lib/redfire-switch";
      description = "Directory where Redfire Switch stores its data.";
    };
    
    logLevel = mkOption {
      type = types.enum [ "error" "warn" "info" "debug" "trace" ];
      default = "info";
      description = "Log level for Redfire Switch.";
    };
    
    openFirewall = mkOption {
      type = types.bool;
      default = false;
      description = "Whether to open the firewall for SIP ports.";
    };
    
    settings = mkOption {
      type = types.attrs;
      default = {};
      description = ''
        Configuration for Redfire Switch.
        See <link xlink:href="https://github.com/carrierone/redfire-switch"/> for available options.
      '';
      example = literalExpression ''
        {
          sip = {
            bind_address = "0.0.0.0:5060";
            external_ip = "203.0.113.1";
            domain = "sip.example.com";
          };
          database = {
            url = "postgresql://redfire:password@localhost/redfire_switch";
          };
          routing = {
            default_route = "carrier1";
          };
        }
      '';
    };
    
    # BGP Anycast Configuration
    bgpAnycast = {
      enable = mkEnableOption "BGP Anycast clustering";
      
      settings = mkOption {
        type = types.attrs;
        default = {};
        description = "BGP Anycast configuration.";
        example = literalExpression ''
          {
            enabled = true;
            node = {
              node_id = "switch1";
              region = "us-east-1";
              priority = 100;
            };
            bgp = {
              router_id = "203.0.113.1";
              local_as = 65001;
              neighbors = [
                {
                  address = "203.0.113.2";
                  remote_as = 65002;
                }
              ];
            };
          }
        '';
      };
    };
    
    # Web Interface Configuration  
    webInterface = {
      enable = mkEnableOption "web management interface";
      
      bind = mkOption {
        type = types.str;
        default = "127.0.0.1:8080";
        description = "Address and port for web interface.";
      };
      
      user = mkOption {
        type = types.str;
        default = "redfire-web";
        description = "User for web interface service.";
      };
    };
    
    # Database Configuration
    database = {
      createLocally = mkOption {
        type = types.bool;
        default = false;
        description = "Whether to create a local PostgreSQL database.";
      };
      
      name = mkOption {
        type = types.str;
        default = "redfire_switch";
        description = "Database name.";
      };
      
      user = mkOption {
        type = types.str;
        default = "redfire";
        description = "Database user.";
      };
    };
  };
  
  ###### Implementation
  
  config = mkIf cfg.enable {
    
    # Default configuration
    services.redfire-switch.settings = mkMerge [
      {
        sip = {
          bind_address = mkDefault "0.0.0.0:5060";
          transport = mkDefault "UDP";
        };
        logging = {
          level = mkDefault cfg.logLevel;
          output = mkDefault "journald";
        };
        paths = {
          data_dir = mkDefault cfg.dataDir;
          log_dir = mkDefault "/var/log/redfire-switch";
        };
      }
      (mkIf cfg.database.createLocally {
        database = {
          url = "postgresql://${cfg.database.user}@localhost/${cfg.database.name}";
        };
      })
    ];
    
    # BGP Anycast settings
    services.redfire-switch.bgpAnycast.settings = mkIf cfg.bgpAnycast.enable {
      enabled = mkDefault true;
      node = {
        node_id = mkDefault config.networking.hostName;
        region = mkDefault "default";
      };
    };
    
    # Users and groups
    users.groups.${cfg.group} = {};
    
    users.users.${cfg.user} = {
      description = "Redfire Switch service user";
      group = cfg.group;
      home = cfg.dataDir;
      createHome = true;
      homeMode = "750";
      isSystemUser = true;
    };
    
    users.users.${cfg.webInterface.user} = mkIf cfg.webInterface.enable {
      description = "Redfire Switch web interface user";
      group = cfg.group;
      home = "${cfg.dataDir}/web";
      createHome = true;
      isSystemUser = true;
    };
    
    # Local PostgreSQL database
    services.postgresql = mkIf cfg.database.createLocally {
      enable = true;
      ensureDatabases = [ cfg.database.name ];
      ensureUsers = [{
        name = cfg.database.user;
        ensurePermissions = {
          "DATABASE \"${cfg.database.name}\"" = "ALL PRIVILEGES";
        };
      }];
    };
    
    # Redis for session storage
    services.redis.servers.redfire-switch = {
      enable = mkDefault true;
      port = mkDefault 6379;
      bind = mkDefault "127.0.0.1";
    };
    
    # Main systemd service
    systemd.services.redfire-switch = {
      description = "Redfire Switch SIP Server";
      documentation = [ "https://github.com/carrierone/redfire-switch" ];
      after = [ "network-online.target" ] 
        ++ optional cfg.database.createLocally "postgresql.service"
        ++ [ "redis-redfire-switch.service" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];
      startLimitIntervalSec = 0;
      
      serviceConfig = {
        Type = "notify";
        ExecStartPre = "${cfg.package}/bin/redfire-switch check-config --config ${configFile}";
        ExecStart = "${cfg.package}/bin/redfire-switch start --config ${configFile}";
        ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
        ExecStop = "${cfg.package}/bin/redfire-switch stop --graceful";
        Restart = "always";
        RestartSec = 5;
        
        User = cfg.user;
        Group = cfg.group;
        
        # Security hardening
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ReadWritePaths = [ cfg.dataDir "/var/log/redfire-switch" "/run/redfire-switch" ];
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
        RestrictNamespaces = true;
        LockPersonality = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        RemoveIPC = true;
        PrivateMounts = true;
        SystemCallFilter = [ "@system-service" ];
        SystemCallErrorNumber = "EPERM";
        
        # Capabilities for privileged ports
        AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" ];
        CapabilityBoundingSet = [ "CAP_NET_BIND_SERVICE" ];
        
        # Resource limits
        LimitNOFILE = 65535;
        LimitNPROC = 32768;
        TasksMax = 4096;
        MemoryMax = "8G";
        
        # Watchdog
        WatchdogSec = 30;
        
        # Environment
        Environment = [
          "RUST_LOG=${cfg.logLevel}"
          "RUST_BACKTRACE=1"
        ];
      };
    };
    
    # BGP Anycast service
    systemd.services.redfire-switch-bgp = mkIf cfg.bgpAnycast.enable {
      description = "Redfire Switch BGP Anycast Service";
      after = [ "redfire-switch.service" ];
      requires = [ "redfire-switch.service" ];
      partOf = [ "redfire-switch.service" ];
      wantedBy = [ "redfire-switch.service" ];
      
      serviceConfig = {
        Type = "notify";
        ExecStartPre = "${cfg.package}/bin/redfire-switch bgp-anycast check-config --config ${bgpConfigFile}";
        ExecStart = "${cfg.package}/bin/redfire-switch bgp-anycast start --config ${bgpConfigFile}";
        ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
        ExecStop = "${cfg.package}/bin/redfire-switch bgp-anycast stop --graceful";
        Restart = "always";
        RestartSec = 5;
        
        User = cfg.user;
        Group = cfg.group;
        
        # Additional capabilities for BGP
        AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" "CAP_NET_RAW" "CAP_NET_ADMIN" ];
        CapabilityBoundingSet = [ "CAP_NET_BIND_SERVICE" "CAP_NET_RAW" "CAP_NET_ADMIN" ];
        
        # Inherit security settings from main service
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ReadWritePaths = [ cfg.dataDir "/var/log/redfire-switch" "/run/redfire-switch" ];
        SystemCallFilter = [ "@system-service" "@network-io" ];
        
        # Environment
        Environment = [
          "RUST_LOG=${cfg.logLevel}"
          "BGP_ANYCAST_ENABLED=1"
        ];
      };
    };
    
    # Web interface service
    systemd.services.redfire-switch-web = mkIf cfg.webInterface.enable {
      description = "Redfire Switch Web Management Interface";
      after = [ "redfire-switch.service" ];
      wants = [ "redfire-switch.service" ];
      wantedBy = [ "multi-user.target" ];
      
      serviceConfig = {
        Type = "notify";
        ExecStart = "${cfg.package}/bin/redfire-switch web-ui --bind ${cfg.webInterface.bind} --config ${configFile}";
        ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
        Restart = "always";
        RestartSec = 5;
        
        User = cfg.webInterface.user;
        Group = cfg.group;
        
        # Web interface security
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ReadWritePaths = [ "${cfg.dataDir}/web" "/var/log/redfire-switch" ];
        AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" ];
        CapabilityBoundingSet = [ "CAP_NET_BIND_SERVICE" ];
        
        # Resource limits
        LimitNOFILE = 8192;
        TasksMax = 512;
        MemoryMax = "512M";
        
        Environment = [
          "RUST_LOG=${cfg.logLevel}"
          "WEB_UI_ENABLED=1"
        ];
      };
    };
    
    # Runtime directories
    systemd.tmpfiles.rules = [
      "d /run/redfire-switch 0755 ${cfg.user} ${cfg.group} -"
      "d /var/log/redfire-switch 0750 ${cfg.user} ${cfg.group} -"
    ];
    
    # Firewall configuration
    networking.firewall = mkIf cfg.openFirewall {
      allowedTCPPorts = [ 5060 5061 ];
      allowedUDPPorts = [ 5060 5061 ];
      allowedUDPPortRanges = [
        { from = 10000; to = 20000; } # RTP media ports
      ];
    };
    
    # Additional firewall rules for web interface
    networking.firewall.allowedTCPPorts = mkIf (cfg.webInterface.enable && cfg.openFirewall) [
      (toInt (last (splitString ":" cfg.webInterface.bind)))
    ];
    
    # Log rotation
    services.logrotate.settings.redfire-switch = {
      files = "/var/log/redfire-switch/*.log";
      frequency = "daily";
      rotate = 30;
      compress = true;
      delaycompress = true;
      missingok = true;
      notifempty = true;
      create = "644 ${cfg.user} ${cfg.group}";
      postrotate = "systemctl reload redfire-switch.service";
    };
    
    # Environment
    environment.systemPackages = [ cfg.package ];
    
    # Assertions
    assertions = [
      {
        assertion = cfg.database.createLocally -> config.services.postgresql.enable;
        message = "PostgreSQL must be enabled when database.createLocally is true";
      }
      {
        assertion = cfg.bgpAnycast.enable -> (cfg.bgpAnycast.settings != {});
        message = "BGP Anycast settings must be configured when enabled";
      }
    ];
    
  };
  
  meta.maintainers = with maintainers; [ /* TODO: add maintainer */ ];
}