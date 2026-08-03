# yx

`yx` is a Rust prototype for the user-facing Why-Ex Yocto/kas workflow tool.

It is intentionally split into two binaries/packages to avoid a dependency loop with `yx-env`:

```text
yx-internal  ──>  no yx-env dependency
     ▲
     │ included in
yx-env
     ▲
     │ consumed by
yx-external
```

Both packages install a binary named `yx`:

- `yx-external`: host-side launcher. It enters the selected `yxenv` environment and re-executes `yx` inside it.
- `yx-internal`: environment-side executor. It runs inside devshell/container and exposes Yocto/kas layer commands.

## Design goal

`yx` should be a transparent layer navigator, not a replacement build system.

It should make these layers explicit:

```text
host system
  -> yxenv environment
    -> kas workspace / kas shell
      -> bitbake / bitbake-layers
        -> recipe devshell
```

## Development environment

Enter the pinned Rust development shell:

```sh
cd yx
nix develop
```

Run checks:

```sh
cargo check --workspace
```

## Build

```sh
cd yx
cargo build
```

Build one side explicitly:

```sh
cargo build -p yx-external
cargo build -p yx-internal
```

## Project config

Copy the example config into a Yocto project:

```sh
mkdir -p .yx
cp path/to/yx/examples/project.toml .yx/project.toml
```

Minimal example:

```toml
[env]
profile = "yocto-scarthgap-kas52"
backend = "nix-develop"
yxenv = "github:why-ex/yx-env"

[kas]
default = "kas/project.yml"
```

## External usage

From the host:

```sh
yx env shell
yx kas dump
yx kas checkout
yx kas shell
yx bitbake core-image-minimal
yx build
yx doctor
```

The external binary maps these to environment entry commands such as:

```sh
nix develop <yxenv-ref>#<profile> -c yx kas dump
```

Use dry-run to avoid black-box behavior:

```sh
yx --dry-run kas dump
```

## Internal usage

Inside `yxenv`, the internal binary provides layer entry and pass-through commands:

```sh
yx env info
yx env shell
yx env exec -- kas --version

yx kas shell
yx kas exec -- bitbake-layers show-layers
yx bitbake core-image-minimal
yx devshell busybox
```

High-level commands are shortcuts over explicit layer commands:

```sh
yx build
```

is equivalent in intent to:

```sh
yx bitbake <default-target>
```

## Current status

This is a general scaffold/prototype. The command mapping is intentionally simple and visible. Before production use, validate exact kas CLI syntax for `kas shell -c` and `kas for-all-repos` against the target kas version.

## Required yx-env integration

For the full design, `yx-env` should eventually:

- include `yx-internal` in selected profiles;
- export environment markers:

```sh
YXENV=1
YXENV_VERSION=<version>
YXENV_PROFILE=<profile>
YXENV_BACKEND=<devshell|container>
YX_LAYER=env
```

- provide a pure extension point so another flake can compose `yx-internal` into environments without relying on impure `YXENV_EXTRA`;
- ensure the internal `yx` appears before any external `yx` in `PATH` inside the environment.
