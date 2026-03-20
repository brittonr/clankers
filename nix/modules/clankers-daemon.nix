{ config, lib, pkgs, ... }:
let
  cfg = config.services.clankers-daemon;
in
{
  options.services.clankers-daemon = {
    enable = lib.mkEnableOption "clankers agent daemon";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The clankers package to use.";
    };

    model = lib.mkOption {
      type = lib.types.str;
      default = "claude-sonnet-4-20250514";
      description = "Default LLM model for agent sessions.";
    };

    allowAll = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Skip token/ACL auth — allow all iroh peers.";
    };

    heartbeat = lib.mkOption {
      type = lib.types.int;
      default = 300;
      description = "Heartbeat interval in seconds (0 to disable).";
    };

    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      description = "Extra arguments passed to `clankers daemon start`.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Environment file with API keys (ANTHROPIC_API_KEY=..., etc).";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "clankers";
      description = "User to run the daemon as.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "clankers";
      description = "Group to run the daemon as.";
    };

    stateDir = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/clankers";
      description = "State directory (used as HOME).";
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.${cfg.user} = {
      isSystemUser = true;
      group = cfg.group;
      home = cfg.stateDir;
      createHome = true;
    };
    users.groups.${cfg.group} = {};

    systemd.services.clankers-daemon = {
      description = "Clankers Agent Daemon";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      environment = {
        HOME = cfg.stateDir;
        RUST_LOG = "info";
        # iroh needs a writable data dir
        IROH_DATA_DIR = "${cfg.stateDir}/.iroh";
      };

      serviceConfig = {
        Type = "simple";
        ExecStart = lib.concatStringsSep " " ([
          "${cfg.package}/bin/clankers"
          "daemon" "start"
          "--model" cfg.model
          "--heartbeat" (toString cfg.heartbeat)
        ]
        ++ lib.optional cfg.allowAll "--allow-all"
        ++ cfg.extraArgs);

        Restart = "on-failure";
        RestartSec = 5;

        User = cfg.user;
        Group = cfg.group;
        StateDirectory = "clankers";
        RuntimeDirectory = "clankers";

        # Hardening
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ReadWritePaths = [ cfg.stateDir ];
        PrivateTmp = true;
      } // lib.optionalAttrs (cfg.environmentFile != null) {
        EnvironmentFile = cfg.environmentFile;
      };
    };
  };
}
