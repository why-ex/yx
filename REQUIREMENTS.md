# yx Requirements

`yx` is the user-facing Why-Ex workflow tool. It is implemented as a Rust workspace and split into external and internal executables to avoid a dependency loop with `yx-env`.

This file is specific to the `yx` project. Common requirements are in `../REQUIREMENTS.md`.

## Project Structure Requirements

### YX-STRUCT-001: Rust workspace

`yx` MUST be a Rust Cargo workspace with these members:

- `crates/yx-common`
- `crates/yx-external`
- `crates/yx-internal`

Verification:

```sh
cd yx
cargo metadata --no-deps
```

### YX-STRUCT-002: Shared common crate

`yx-common` MUST contain shared functionality used by both binaries, including project configuration discovery, environment detection, command specification/printing, shell quoting, and protocol constants.

Verification:

- Inspect `crates/yx-common/src/lib.rs`.
- Confirm both `yx-external` and `yx-internal` depend on `yx-common`.

### YX-STRUCT-003: Binary names

Both `yx-external` and `yx-internal` MUST build a binary named `yx`.

Verification:

```sh
cd yx
cargo build -p yx-external
cargo build -p yx-internal
```

Then inspect generated binaries under `target/debug/yx` for each package build context.

## Development Environment Requirements

### YX-DEV-001: Nix development shell

`yx` MUST provide a flake development shell containing Rust development tools.

Required tools:

- `cargo`
- `rustc`
- `rustfmt`
- `clippy`
- `rust-analyzer`

Verification:

```sh
cd yx
nix develop -c cargo --version
nix develop -c rustc --version
nix develop -c cargo check --workspace
```

### YX-DEV-002: Formatting support

`yx` SHOULD expose a Nix formatter and source formatting SHOULD be possible with `cargo fmt`.

Verification:

```sh
cd yx
nix flake check --no-build
nix develop -c cargo fmt --check
```

## Configuration Requirements

### YX-CONFIG-001: Project config discovery

`yx` MUST discover `.yx/project.toml` by searching upward from the current working directory. If no config is found, it MUST use documented defaults.

Default values:

```toml
[env]
profile = "yocto-scarthgap-kas52"
backend = "nix-develop"
yxenv = "github:why-ex/yx-env"

[kas]
default = "kas/project.yml"

[paths]
downloads = ".yx/downloads"
sstate = ".yx/sstate"
build = "build"

[build]
default_target = "core-image-minimal"
```

Verification:

- Run `yx env info` or `yx --dry-run kas dump` with and without `.yx/project.toml`.
- Confirm configured values override defaults.

### YX-CONFIG-002: Execution-policy-only config

The config parser MUST only treat `.yx/project.toml` as yx execution policy. It MUST NOT implement a parallel kas/YAML replacement for Yocto layers, remotes, branches, recipes, or distro/machine configuration.

Verification:

- Inspect accepted keys in `yx-common`.
- Confirm unknown keys are ignored or rejected without affecting Yocto topology.

### YX-CONFIG-003: Example config

The repository MUST include an example project configuration at `examples/project.toml`.

Verification:

```sh
test -f yx/examples/project.toml
```

## Environment Detection Requirements

### YX-ENV-001: Detect `yx-env` context

`yx-common` MUST detect these environment variables:

- `YXENV`
- `YXENV_PROFILE`
- `YXENV_VERSION`
- `YXENV_BACKEND`
- `YX_LAYER`

Verification:

- Inspect `EnvState::detect`.
- Run with controlled environment variables and confirm `yx env info` reports them inside the internal binary.

### YX-ENV-002: External binary must not run as internal executor

When `yx-external` detects `YXENV=1`, it MUST refuse to act as the internal executor and return non-zero, because the internal binary should be first in `PATH` inside `yx-env`.

Verification:

```sh
cd yx
YXENV=1 nix develop -c cargo run -p yx-external -- env info
```

Expected: diagnostic explaining that `yx-external` is running inside `yx-env`, with non-zero exit.

## External Launcher Requirements

### YX-EXT-001: Host-side role

`yx-external` MUST run on the host and enter the configured environment before executing Yocto/kas workflow commands.

Verification:

```sh
cd yx
cargo run -p yx-external -- --dry-run kas dump
```

Expected: printed command starts with `nix develop <yxenv-ref>#<profile> -c yx ...` for the default backend.

### YX-EXT-002: Supported external commands

`yx-external` MUST support these command families:

```sh
yx env info
yx env shell
yx env exec -- <command> [args...]
yx kas ...
yx bitbake ...
yx build ...
yx devshell ...
yx manifest ...
yx doctor
```

Verification:

- Run each family with `--dry-run` where applicable.
- Unknown commands MUST return non-zero.

### YX-EXT-003: `nix-develop` backend

For backend `nix-develop`, `yx-external` MUST map commands to:

```sh
nix develop <yxenv-ref>#<profile> -c yx <original-args...>
```

For `yx env shell`, it MUST enter the selected development shell without adding a re-exec command.

Verification:

```sh
yx --dry-run kas dump
yx --dry-run env shell
```

### YX-EXT-004: Container backend

For backend `container`, `docker`, or `podman`, `yx-external` MUST require `[env].image` in `.yx/project.toml`. It MUST generate a container run command that:

- runs interactively and removes the container afterward;
- maps current UID/GID;
- mounts current working directory at the same path;
- mounts `/tmp` and `/var/tmp`;
- sets the working directory to the current working directory;
- runs `yx <original-args...>` inside the container, or `bash` for a plain shell.

Verification:

- Create temporary `.yx/project.toml` with container backend and image.
- Run `yx --dry-run kas dump` and inspect the generated command.
- Remove `image` and confirm a non-zero error.

### YX-EXT-005: Dry-run behavior

`yx-external` MUST support `--dry-run` and `--print-command` as aliases. These flags MUST print the command that would be executed and MUST NOT execute it.

Verification:

```sh
yx --dry-run kas dump
yx --print-command kas dump
```

## Internal Executor Requirements

### YX-INT-001: Internal role

`yx-internal` MUST run inside `yx-env` and expose workflow/layer commands. It MUST NOT enter `yx-env` itself.

Verification:

- Inspect `yx-internal`: it invokes `kas`, `bitbake`, shells, and diagnostic commands directly rather than invoking `nix develop` or container runtimes.

### YX-INT-002: Internal handshake

`yx-internal` MUST support:

```sh
yx --internal-handshake
```

It MUST print machine-readable output containing:

- `kind = yx-internal`
- version
- protocol
- capabilities

Verification:

```sh
cd yx
cargo run -p yx-internal -- --internal-handshake
```

### YX-INT-003: Environment layer commands

`yx-internal` MUST support:

```sh
yx env info
yx env shell
yx env exec -- <command> [args...]
yx env doctor
```

Verification:

- `yx env info` prints environment state and project config.
- `yx env exec -- echo ok` runs or dry-runs the command.
- `yx env shell` starts `$SHELL` or `bash` and sets `YX_LAYER=env` for the child command.

### YX-INT-004: kas layer commands

`yx-internal` MUST support:

```sh
yx kas shell [kas.yml]
yx kas exec -- <command> [args...]
yx kas build [kas.yml]
yx kas checkout [kas.yml]
yx kas dump [kas.yml]
yx kas lock [kas.yml]
yx kas menu [kas.yml]
yx kas clean [kas.yml]
yx kas cleanall [kas.yml]
yx kas purge [kas.yml]
yx kas for-all-repos [kas.yml] -- <command>
```

Verification:

- Use `--dry-run` to verify generated kas commands without requiring a full Yocto checkout.
- In a real kas project, execute representative commands.

### YX-INT-005: BitBake shortcuts

`yx-internal` MUST provide shortcuts for BitBake through the kas context:

```sh
yx bitbake <args...>
yx build [target]
yx devshell <recipe>
```

Expected mappings:

- `yx bitbake <args...>` maps to running `bitbake <args...>` in the kas context.
- `yx build [target]` maps to `yx bitbake <target>`, using the configured default target if none is provided.
- `yx devshell <recipe>` maps to `bitbake -c devshell <recipe>` in the kas context.

Verification:

```sh
yx --dry-run bitbake core-image-minimal
yx --dry-run build
yx --dry-run devshell busybox
```

### YX-INT-006: Manifest command

`yx-internal` SHOULD provide:

```sh
yx manifest [kas.yml]
```

The command SHOULD use kas repository context to print repository names and revisions.

Verification:

- Use `--dry-run` to inspect the generated `kas for-all-repos` command.
- In a checked-out kas workspace, confirm it prints repository identifiers and commit hashes.

### YX-INT-007: Doctor command

`yx-internal` MUST provide:

```sh
yx doctor
```

It MUST check for required tools and return non-zero when required internal tools are missing. At minimum it MUST check `git`, `kas`, `python3`, and `bash` as required. It SHOULD also report `bitbake` and `bitbake-layers` availability.

Verification:

- Run inside a valid `yx-env` profile.
- Run in a minimal or intentionally incomplete environment and confirm failure for missing required tools.

### YX-INT-008: Dry-run behavior

`yx-internal` MUST support `--dry-run` and `--print-command`. These flags MUST print the underlying command and MUST NOT execute it.

Verification:

```sh
yx --dry-run kas dump
yx --print-command bitbake core-image-minimal
```

## Command Visibility Requirements

### YX-CMD-001: Printed commands

All command execution paths through `run_or_print` MUST print the command with a `+ ` prefix before execution or dry-run completion.

Verification:

- Run representative commands and confirm output includes `+ <command>`.

### YX-CMD-002: Exit codes

When an underlying command exits with a status code, `yx` MUST return that code. If command execution fails because the program is not found or cannot be started, `yx` MUST return `127`.

Verification:

- Run `yx env exec -- false` inside internal mode and confirm exit code `1`.
- Run `yx env exec -- definitely-missing-command` and confirm exit code `127`.

## Testing Requirements

### YX-TEST-001: Workspace check

The workspace MUST pass Rust type checking.

Verification:

```sh
cd yx
nix develop -c cargo check --workspace
```

### YX-TEST-002: Help output

Both binaries MUST provide help/usage output for empty command or `--help`.

Verification:

```sh
cargo run -p yx-external -- --help
cargo run -p yx-internal -- --help
```

### YX-TEST-003: No Yocto checkout required for basic tests

Basic parser, help, handshake, and dry-run tests MUST NOT require a Yocto checkout.

Verification:

- Run help, handshake, `env info`, and `--dry-run` tests from an arbitrary directory.
