{ config, lib, pkgs, ... }:
let
  cfg = config.services.clanker-router;
in
{
  options.services.clanker-router = {
    enable = lib.mkEnableOption "clanker-router LLM proxy";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The clanker-router package to use.";
    };

    proxyAddr = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:4000";
      description = "HTTP proxy listen address.";
    };

    proxyKeys = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      description = "API keys allowed to access the proxy. Empty = no auth.";
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Open the proxy port in the firewall.";
    };

    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      description = "Extra arguments passed to `clanker-router serve`.";
    };

    localProviderConfig = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "JSON file describing extra OpenAI-compatible providers to register.";
    };

    authFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Path to a clanker-router auth.json file.
        Supports Anthropic OAuth plus API-key provider entries in one store.
      '';
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

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Environment file with provider API keys. One per line:
          ANTHROPIC_API_KEY=sk-ant-...
          OPENAI_API_KEY=sk-...
          OPENROUTER_API_KEY=sk-or-...
      '';
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "clanker-router";
      description = "User to run the router as.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "clanker-router";
      description = "Group to run the router as.";
    };

    stateDir = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/clanker-router";
      description = "State directory (auth store, database, iroh identity).";
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

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [
      (lib.toInt (lib.last (lib.splitString ":" cfg.proxyAddr)))
    ];

    systemd.services.clanker-router = {
      description = "Clanker Router LLM Proxy";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      environment = {
        HOME = cfg.stateDir;
        RUST_LOG = "clanker_router=info";
      };

      serviceConfig = {
        Type = "simple";
        ExecStart = lib.concatStringsSep " " ([
          "${cfg.package}/bin/clanker-router"
        ]
        ++ lib.optional (cfg.authFile != null) "--auth-file ${cfg.authFile}"
        ++ lib.optional (cfg.authSeedFile != null) "--auth-seed-file ${cfg.authSeedFile}"
        ++ lib.optional (cfg.authRuntimeFile != null) "--auth-runtime-file ${cfg.authRuntimeFile}"
        ++ lib.optional (cfg.localProviderConfig != null) "--local-provider-config ${cfg.localProviderConfig}"
        ++ [
          "serve"
          "--proxy-addr" cfg.proxyAddr
        ]
        ++ lib.concatMap (k: [ "--proxy-key" k ]) cfg.proxyKeys
        ++ cfg.extraArgs);

        Restart = "on-failure";
        RestartSec = 5;

        User = cfg.user;
        Group = cfg.group;
        StateDirectory = "clanker-router";
        RuntimeDirectory = "clanker-router";

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
