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

    pluginsPackage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = "Nix-built WASM plugins package (from clankers-plugins). When set, plugins are symlinked into the daemon's state directory so they're always in sync with the source.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Environment file with API keys (ANTHROPIC_API_KEY=..., etc).";
    };

    authFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Single auth store path override for service deployments.";
    };

    authSeedFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Read-only seed auth store path for managed service deployments.";
    };

    authRuntimeFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Writable runtime auth store path for managed service deployments.";
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

    # Symlink nix-built plugins into the daemon's plugin discovery path.
    # The daemon scans ~/.clankers/agent/plugins/ at startup.
    systemd.tmpfiles.rules = lib.mkIf (cfg.pluginsPackage != null) (
      let
        pluginsSrc = "${cfg.pluginsPackage}/lib/clankers/plugins";
        pluginsDest = "${cfg.stateDir}/.clankers/agent/plugins";
      in [
        "d ${cfg.stateDir}/.clankers 0755 ${cfg.user} ${cfg.group} -"
        "d ${cfg.stateDir}/.clankers/agent 0755 ${cfg.user} ${cfg.group} -"
        "L+ ${pluginsDest} - - - - ${pluginsSrc}"
      ]
    );

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
        # Model selection — daemon reads from env when no CLI flag
        CLANKERS_MODEL = cfg.model;
      }
      // lib.optionalAttrs (cfg.authFile != null) {
        CLANKERS_AUTH_FILE = toString cfg.authFile;
      }
      // lib.optionalAttrs (cfg.authSeedFile != null) {
        CLANKERS_AUTH_SEED_FILE = toString cfg.authSeedFile;
      }
      // lib.optionalAttrs (cfg.authRuntimeFile != null) {
        CLANKERS_AUTH_RUNTIME_FILE = toString cfg.authRuntimeFile;
      };

      serviceConfig = {
        Type = "simple";
        ExecStart = lib.concatStringsSep " " ([
          "${cfg.package}/bin/clankers"
          "daemon" "start"
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
