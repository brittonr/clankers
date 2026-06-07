# Design: Isolate Runtime Adapter Shells

## Boundary

Runtime modules should fall into one of three buckets:

1. green contract DTOs/traits with no ambient desktop dependencies;
2. yellow host-injection surfaces that fail closed without services;
3. desktop adapter implementations that may read config, filesystem, auth stores, plugin roots, or shell state.

The public inventory should record the bucket for each row. New runtime APIs cannot be promoted as embedded SDK contracts unless they are in bucket 1 or have an explicit optional-support host-injection contract.

## Migration path

Start with classification and rail hardening, then move one concern at a time. Avoid a broad runtime crate rewrite; keep adapter shims until downstream root/controller code is migrated.
