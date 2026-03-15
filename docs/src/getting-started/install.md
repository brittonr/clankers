# Installation

## From source

```bash
git clone https://github.com/brittonr/clankers
cd clankers
cargo build --release
```

The binary lands at `target/release/clankers`.

## With Nix

The project includes a `flake.nix`:

```bash
nix build
# or enter a devshell
nix develop
```

## Verify

```bash
clankers --version
cargo nextest run          # run the test suite
cargo clippy -- -D warnings
```
