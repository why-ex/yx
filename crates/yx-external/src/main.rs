use std::env;
use std::process;

use yx_common::{
    current_uid_gid, os_args_without_program, run_or_print, CmdSpec, EnvState, ProjectConfig,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let mut args = os_args_without_program();
    let dry_run = take_flag(&mut args, "--dry-run") || take_flag(&mut args, "--print-command");

    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        usage();
        return;
    }

    let cfg = ProjectConfig::discover();
    let state = EnvState::detect();

    let code = if state.inside {
        already_inside_message(&args);
        2
    } else {
        match args[0].as_str() {
            "env" => env_cmd(&cfg, &args[1..], dry_run),
            "shell" => enter_env_shell(&cfg, dry_run),
            "doctor" => enter_and_reexec(&cfg, &args, dry_run),
            "kas" | "bitbake" | "build" | "devshell" | "manifest" => enter_and_reexec(&cfg, &args, dry_run),
            other => {
                eprintln!("yx-external: unknown command: {other}");
                usage();
                2
            }
        }
    };

    process::exit(code);
}

fn env_cmd(cfg: &ProjectConfig, args: &[String], dry_run: bool) -> i32 {
    match args.first().map(String::as_str) {
        Some("info") | None => print_external_info(cfg),
        Some("shell") => enter_env_shell(cfg, dry_run),
        Some("exec") => enter_and_reexec(cfg, &prepend("env", args), dry_run),
        Some("doctor") => enter_and_reexec(cfg, &vec!["doctor".to_string()], dry_run),
        Some(other) => {
            eprintln!("yx env: unknown subcommand: {other}");
            2
        }
    }
}

fn enter_env_shell(cfg: &ProjectConfig, dry_run: bool) -> i32 {
    match cfg.backend.as_str() {
        "container" | "docker" | "podman" => {
            let runtime = if cfg.backend == "podman" { "podman" } else { "docker" };
            container_cmd(cfg, runtime, &[]).map_or_else(
                |err| {
                    eprintln!("yx: {err}");
                    2
                },
                |cmd| run_or_print(&cmd, dry_run),
            )
        }
        _ => run_or_print(&nix_develop_shell_cmd(cfg), dry_run),
    }
}

fn enter_and_reexec(cfg: &ProjectConfig, args: &[String], dry_run: bool) -> i32 {
    match cfg.backend.as_str() {
        "container" | "docker" | "podman" => {
            let runtime = if cfg.backend == "podman" { "podman" } else { "docker" };
            container_cmd(cfg, runtime, args).map_or_else(
                |err| {
                    eprintln!("yx: {err}");
                    2
                },
                |cmd| run_or_print(&cmd, dry_run),
            )
        }
        _ => run_or_print(&nix_develop_reexec_cmd(cfg, args), dry_run),
    }
}

fn nix_develop_shell_cmd(cfg: &ProjectConfig) -> CmdSpec {
    CmdSpec::new("nix")
        .arg("develop")
        .arg(format!("{}#{}", cfg.yxenv_ref, cfg.profile))
        .cwd(cfg.root.clone())
}

fn nix_develop_reexec_cmd(cfg: &ProjectConfig, args: &[String]) -> CmdSpec {
    CmdSpec::new("nix")
        .arg("develop")
        .arg(format!("{}#{}", cfg.yxenv_ref, cfg.profile))
        .arg("-c")
        .arg("yx")
        .args(args.iter().cloned())
        .cwd(cfg.root.clone())
}

fn container_cmd(cfg: &ProjectConfig, runtime: &str, args: &[String]) -> Result<CmdSpec, String> {
    let Some(image) = cfg.image.clone() else {
        return Err("container backend requires [env].image in .yx/project.toml".to_string());
    };

    let cwd = env::current_dir().unwrap_or_else(|_| cfg.root.clone());
    let (uid, gid) = current_uid_gid();
    let mut cmd = CmdSpec::new(runtime)
        .arg("run")
        .arg("--rm")
        .arg("-ti")
        .arg("-u")
        .arg(format!("{uid}:{gid}"))
        .arg("-v")
        .arg(format!("{}:{}:rw", cwd.display(), cwd.display()))
        .arg("-v")
        .arg("/tmp:/tmp:rw")
        .arg("-v")
        .arg("/var/tmp:/var/tmp:rw")
        .arg("--workdir")
        .arg(cwd.display().to_string())
        .arg(image);

    if args.is_empty() {
        cmd = cmd.arg("bash");
    } else {
        cmd = cmd.arg("yx").args(args.iter().cloned());
    }
    Ok(cmd)
}

fn print_external_info(cfg: &ProjectConfig) -> i32 {
    println!("yx-external {}", VERSION);
    println!("project root: {}", cfg.root.display());
    println!("backend: {}", cfg.backend);
    println!("yxenv ref: {}", cfg.yxenv_ref);
    println!("profile: {}", cfg.profile);
    println!("default kas file: {}", cfg.kas_default);
    println!("default target: {}", cfg.default_target);
    if let Some(image) = &cfg.image {
        println!("container image: {}", image);
    }
    0
}

fn already_inside_message(args: &[String]) {
    eprintln!("yx-external: YXENV=1 is set, so this appears to be inside yxenv.");
    eprintln!("yx-external should normally run on the host, while yx-internal should be first in PATH inside yxenv.");
    eprintln!("requested command: yx {}", args.join(" "));
}

fn prepend(head: &str, tail: &[String]) -> Vec<String> {
    let mut out = vec![head.to_string()];
    out.extend_from_slice(tail);
    out
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(pos) = args.iter().position(|a| a == flag) {
        args.remove(pos);
        true
    } else {
        false
    }
}

fn usage() {
    eprintln!(
        "yx-external {VERSION}\n\
         Runs on the host. Enters yxenv and re-executes yx-internal.\n\n\
         Commands:\n\
           yx env info\n\
           yx env shell           # enter bare yxenv\n\
           yx env exec -- <cmd>   # run command inside yxenv\n\
           yx kas shell [kas.yml]\n\
           yx kas dump [kas.yml]\n\
           yx kas checkout [kas.yml]\n\
           yx bitbake <args...>\n\
           yx build [target]\n\
           yx devshell <recipe>\n\
           yx manifest [kas.yml]\n\
           yx doctor\n\n\
         Use --dry-run to print the environment entry command."
    );
}
