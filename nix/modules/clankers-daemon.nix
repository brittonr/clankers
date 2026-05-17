{ config, lib, pkgs, ... }:
let
  cfg = config.services.clankers-daemon;
  processCfg = cfg.processManagement;
  retentionCfg = processCfg.retention;
  pueueCfg = processCfg.pueue;
  pueueGroupSetupCommands = lib.concatLines (lib.mapAttrsToList (name: concurrency: ''
    ${pueueCfg.package}/bin/pueue group add ${lib.escapeShellArg name} >/dev/null 2>&1 || true
    ${pueueCfg.package}/bin/pueue parallel --group ${lib.escapeShellArg name} ${toString concurrency}
  '') pueueCfg.groups);
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

    processManagement = {
      enable = lib.mkEnableOption "durable Clankers process/job persistence";

      defaultBackend = lib.mkOption {
        type = lib.types.enum [ "native" "pueue" "systemd" ];
        default = "native";
        description = "Default durable process/job backend used when a tool request does not specify one.";
      };

      databasePath = lib.mkOption {
        type = lib.types.str;
        default = "${cfg.stateDir}/process-jobs/process-jobs.redb";
        description = "redb database path used for durable process/job registry persistence.";
      };

      logDir = lib.mkOption {
        type = lib.types.str;
        default = "${cfg.stateDir}/process-jobs/logs";
        description = "Directory for durable process/job log chunks and backend log references.";
      };

      registryDir = lib.mkOption {
        type = lib.types.str;
        default = "${cfg.stateDir}/process-jobs";
        description = "Directory that owns durable process/job registry metadata.";
      };

      retention = {
        maxAgeDays = lib.mkOption {
          type = lib.types.ints.positive;
          default = 14;
          description = "Maximum age in days for completed durable process/job records before garbage collection may remove them.";
        };

        maxRecords = lib.mkOption {
          type = lib.types.ints.positive;
          default = 1000;
          description = "Maximum completed process/job records retained before garbage collection may prune old entries.";
        };

        maxLogBytes = lib.mkOption {
          type = lib.types.ints.positive;
          default = 1073741824;
          description = "Maximum total process/job log bytes retained before garbage collection may prune old logs.";
        };
      };

      pueue = {
        enable = lib.mkEnableOption "pueue durable process/job backend integration";

        package = lib.mkOption {
          type = lib.types.package;
          default = pkgs.pueue;
          defaultText = lib.literalExpression "pkgs.pueue";
          description = "pueue package used for the optional managed daemon and Clankers pueue backend calls.";
        };

        manageService = lib.mkOption {
          type = lib.types.bool;
          default = true;
          description = "Whether this module manages a pueued system service for Clankers. Disable when an existing compatible pueue service is provided separately.";
        };

        stateDir = lib.mkOption {
          type = lib.types.str;
          default = "${cfg.stateDir}/pueue";
          description = "Writable HOME/state directory used by the managed pueue service.";
        };

        groups = lib.mkOption {
          type = lib.types.attrsOf lib.types.ints.unsigned;
          default = { clankers = 4; };
          description = "pueue groups and deterministic parallelism limits materialized by the module.";
        };
      };
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

    assertions = [
      {
        assertion = processCfg.defaultBackend != "pueue" || pueueCfg.enable;
        message = "services.clankers-daemon.processManagement.defaultBackend = \"pueue\" requires processManagement.pueue.enable = true.";
      }
    ];

    systemd.tmpfiles.rules =
      (lib.optionals (cfg.pluginsPackage != null) (
        let
          pluginsSrc = "${cfg.pluginsPackage}/lib/clankers/plugins";
          pluginsDest = "${cfg.stateDir}/.clankers/agent/plugins";
        in [
          "d ${cfg.stateDir}/.clankers 0755 ${cfg.user} ${cfg.group} -"
          "d ${cfg.stateDir}/.clankers/agent 0755 ${cfg.user} ${cfg.group} -"
          "L+ ${pluginsDest} - - - - ${pluginsSrc}"
        ]
      ))
      ++ lib.optionals processCfg.enable [
        "d ${processCfg.registryDir} 0750 ${cfg.user} ${cfg.group} -"
        "d ${processCfg.logDir} 0750 ${cfg.user} ${cfg.group} -"
      ]
      ++ lib.optionals (processCfg.enable && pueueCfg.enable) [
        "d ${pueueCfg.stateDir} 0750 ${cfg.user} ${cfg.group} -"
      ];

    systemd.services.clankers-pueued = lib.mkIf (processCfg.enable && pueueCfg.enable && pueueCfg.manageService) {
      description = "Clankers managed pueue daemon";
      wantedBy = [ "multi-user.target" ];
      before = [ "clankers-daemon.service" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${pueueCfg.package}/bin/pueued";
        User = cfg.user;
        Group = cfg.group;
        Restart = "on-failure";
        RestartSec = 5;
        WorkingDirectory = pueueCfg.stateDir;
        ReadWritePaths = [ pueueCfg.stateDir ];
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
      };
      environment = {
        HOME = pueueCfg.stateDir;
        PUEUE_CONFIG_PATH = "${pueueCfg.stateDir}/pueue.yml";
      };
    };

    systemd.services.clankers-pueue-setup = lib.mkIf (processCfg.enable && pueueCfg.enable && pueueCfg.manageService) {
      description = "Configure Clankers pueue groups and concurrency";
      after = [ "clankers-pueued.service" ];
      requires = [ "clankers-pueued.service" ];
      before = [ "clankers-daemon.service" ];
      serviceConfig = {
        Type = "oneshot";
        User = cfg.user;
        Group = cfg.group;
      };
      environment = {
        HOME = pueueCfg.stateDir;
        PUEUE_CONFIG_PATH = "${pueueCfg.stateDir}/pueue.yml";
      };
      script = ''
        for _ in $(seq 1 50); do
          if ${pueueCfg.package}/bin/pueue status >/dev/null 2>&1; then
            break
          fi
          sleep 0.1
        done
        ${pueueGroupSetupCommands}
      '';
    };

    systemd.services.clankers-daemon = {
      description = "Clankers Agent Daemon";
      after = [ "network-online.target" ] ++ lib.optionals (processCfg.enable && pueueCfg.enable && pueueCfg.manageService) [ "clankers-pueue-setup.service" ];
      wants = [ "network-online.target" ] ++ lib.optionals (processCfg.enable && pueueCfg.enable && pueueCfg.manageService) [ "clankers-pueued.service" ];
      wantedBy = [ "multi-user.target" ];

      environment = {
        HOME = cfg.stateDir;
        RUST_LOG = "info";
        # iroh needs a writable data dir
        IROH_DATA_DIR = "${cfg.stateDir}/.iroh";
        # Model selection — daemon reads from env when no CLI flag
        CLANKERS_MODEL = cfg.model;
      }
      // lib.optionalAttrs processCfg.enable {
        CLANKERS_PROCESS_JOBS_ENABLED = "1";
        CLANKERS_PROCESS_JOB_DEFAULT_BACKEND = processCfg.defaultBackend;
        CLANKERS_PROCESS_JOB_DB = processCfg.databasePath;
        CLANKERS_PROCESS_JOB_REGISTRY_DIR = processCfg.registryDir;
        CLANKERS_PROCESS_JOB_LOG_DIR = processCfg.logDir;
        CLANKERS_PROCESS_JOB_RETENTION_MAX_AGE_DAYS = toString retentionCfg.maxAgeDays;
        CLANKERS_PROCESS_JOB_RETENTION_MAX_RECORDS = toString retentionCfg.maxRecords;
        CLANKERS_PROCESS_JOB_RETENTION_MAX_LOG_BYTES = toString retentionCfg.maxLogBytes;
      }
      // lib.optionalAttrs (processCfg.enable && pueueCfg.enable) {
        CLANKERS_PROCESS_JOB_PUEUE_ENABLED = "1";
        CLANKERS_PROCESS_JOB_PUEUE_GROUPS = builtins.concatStringsSep "," (builtins.attrNames pueueCfg.groups);
        PUEUE_CONFIG_PATH = "${pueueCfg.stateDir}/pueue.yml";
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
        ReadWritePaths = [ cfg.stateDir ] ++ lib.optionals processCfg.enable [
          processCfg.registryDir
          processCfg.logDir
          (builtins.dirOf processCfg.databasePath)
        ] ++ lib.optionals (processCfg.enable && pueueCfg.enable) [
          pueueCfg.stateDir
        ];
        PrivateTmp = true;
      } // lib.optionalAttrs (cfg.environmentFile != null) {
        EnvironmentFile = cfg.environmentFile;
      };
    };
  };
}
