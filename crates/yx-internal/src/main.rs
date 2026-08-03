use std::env;
use std::process;

use yx_common::{
    command_exists, json_escape, join_shell_words, os_args_without_program, run_or_print,
    split_after_double_dash, CmdSpec, EnvState, ProjectConfig, YX_PROTOCOL,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let mut args = os_args_without_program();
    let dry_run = take_flag(&mut args, "--dry-run") || take_flag(&mut args, "--print-command");

    if args.is_empty() {
        usage();
        return;
    }

    let cfg = ProjectConfig::discover();
    let code = match args[0].as_str() {
        "--internal-handshake" | "__handshake" => handshake(),
        "env" => env_cmd(&cfg, &args[1..], dry_run),
        "kas" => kas_cmd(&cfg, &args[1..], dry_run),
        "bitbake" => bitbake_cmd(&cfg, &args[1..], dry_run),
        "devshell" => devshell_cmd(&cfg, &args[1..], dry_run),
        "build" => build_cmd(&cfg, &args[1..], dry_run),
        "manifest" => manifest_cmd(&cfg, &args[1..], dry_run),
        "doctor" => doctor_cmd(&cfg),
        "help" | "--help" | "-h" => {
            usage();
            0
        }
        other => {
            eprintln!("yx-internal: unknown command: {other}");
            usage();
            2
        }
    };

    process::exit(code);
}

fn env_cmd(cfg: &ProjectConfig, args: &[String], dry_run: bool) -> i32 {
    match args.first().map(String::as_str) {
        Some("info") | None => env_info(cfg),
        Some("doctor") => doctor_cmd(cfg),
        Some("shell") => {
            let shell = env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
            run_or_print(&CmdSpec::new(shell).env("YX_LAYER", "env"), dry_run)
        }
        Some("exec") => {
            let cmd = split_after_double_dash(&args[1..]);
            run_passthrough(&cmd, "env", dry_run)
        }
        Some(other) => {
            eprintln!("yx env: unknown subcommand: {other}");
            2
        }
    }
}

fn kas_cmd(cfg: &ProjectConfig, args: &[String], dry_run: bool) -> i32 {
    match args.first().map(String::as_str) {
        Some("shell") => {
            let kas_file = cfg.kas_file(args.get(1));
            run_or_print(&CmdSpec::new("kas").arg("shell").arg(kas_file).env("YX_LAYER", "kas"), dry_run)
        }
        Some("exec") => {
            let cmd = split_after_double_dash(&args[1..]);
            if cmd.is_empty() {
                eprintln!("usage: yx kas exec -- <command> [args...]");
                return 2;
            }
            let command_line = join_shell_words(&cmd);
            run_or_print(
                &CmdSpec::new("kas")
                    .arg("shell")
                    .arg(cfg.kas_default.clone())
                    .arg("-c")
                    .arg(command_line)
                    .env("YX_LAYER", "kas"),
                dry_run,
            )
        }
        Some("build" | "checkout" | "dump" | "lock" | "menu" | "clean" | "cleanall" | "purge") => {
            let subcmd = args[0].clone();
            let kas_file = cfg.kas_file(args.get(1));
            run_or_print(&CmdSpec::new("kas").arg(subcmd).arg(kas_file), dry_run)
        }
        Some("for-all-repos") => {
            let rest = split_after_double_dash(&args[1..]);
            if rest.is_empty() {
                eprintln!("usage: yx kas for-all-repos [kas.yml] -- <command>");
                return 2;
            }
            run_or_print(
                &CmdSpec::new("kas")
                    .arg("for-all-repos")
                    .arg(cfg.kas_default.clone())
                    .arg(join_shell_words(&rest)),
                dry_run,
            )
        }
        Some(other) => {
            eprintln!("yx kas: unknown subcommand: {other}");
            2
        }
        None => {
            eprintln!("usage: yx kas <shell|exec|build|checkout|dump|lock|for-all-repos> ...");
            2
        }
    }
}

fn bitbake_cmd(cfg: &ProjectConfig, args: &[String], dry_run: bool) -> i32 {
    let mut cmd = vec!["bitbake".to_string()];
    cmd.extend_from_slice(args);
    let command_line = join_shell_words(&cmd);
    run_or_print(
        &CmdSpec::new("kas")
            .arg("shell")
            .arg(cfg.kas_default.clone())
            .arg("-c")
            .arg(command_line)
            .env("YX_LAYER", "kas"),
        dry_run,
    )
}

fn devshell_cmd(cfg: &ProjectConfig, args: &[String], dry_run: bool) -> i32 {
    let Some(recipe) = args.first() else {
        eprintln!("usage: yx devshell <recipe>");
        return 2;
    };
    bitbake_cmd(cfg, &["-c".to_string(), "devshell".to_string(), recipe.clone()], dry_run)
}

fn build_cmd(cfg: &ProjectConfig, args: &[String], dry_run: bool) -> i32 {
    let target = args.first().cloned().unwrap_or_else(|| cfg.default_target.clone());
    bitbake_cmd(cfg, &[target], dry_run)
}

fn manifest_cmd(cfg: &ProjectConfig, args: &[String], dry_run: bool) -> i32 {
    let kas_file = cfg.kas_file(args.first());
    run_or_print(
        &CmdSpec::new("kas")
            .arg("for-all-repos")
            .arg(kas_file)
            .arg("printf '%s ' \"$KAS_REPO_NAME\"; git rev-parse HEAD"),
        dry_run,
    )
}

fn env_info(cfg: &ProjectConfig) -> i32 {
    let state = EnvState::detect();
    println!("yx-internal {}", VERSION);
    println!("protocol: {}", YX_PROTOCOL);
    println!("inside yxenv: {}", state.inside);
    println!("yxenv profile: {}", state.profile.as_deref().unwrap_or("<unknown>"));
    println!("yxenv version: {}", state.version.as_deref().unwrap_or("<unknown>"));
    println!("yxenv backend: {}", state.backend.as_deref().unwrap_or("<unknown>"));
    println!("current layer: {}", state.layer.as_deref().unwrap_or("env"));
    println!("project root: {}", cfg.root.display());
    println!("configured profile: {}", cfg.profile);
    println!("default kas file: {}", cfg.kas_default);
    0
}

fn doctor_cmd(_cfg: &ProjectConfig) -> i32 {
    let checks = ["git", "kas", "python3", "bitbake", "bitbake-layers", "bash"];
    let mut ok = true;
    println!("yx doctor: internal environment checks");
    for name in checks {
        let exists = command_exists(name);
        println!("  {:16} {}", name, if exists { "ok" } else { "missing" });
        if matches!(name, "git" | "kas" | "python3" | "bash") && !exists {
            ok = false;
        }
    }
    if ok { 0 } else { 1 }
}

fn run_passthrough(cmd: &[String], layer: &str, dry_run: bool) -> i32 {
    if cmd.is_empty() {
        eprintln!("usage: yx env exec -- <command> [args...]");
        return 2;
    }
    run_or_print(&CmdSpec::new(cmd[0].clone()).args(cmd[1..].iter().cloned()).env("YX_LAYER", layer), dry_run)
}

fn handshake() -> i32 {
    println!(
        "{{\"kind\":\"yx-internal\",\"version\":\"{}\",\"protocol\":{},\"capabilities\":[\"env\",\"kas\",\"bitbake\",\"manifest\"]}}",
        json_escape(VERSION),
        YX_PROTOCOL
    );
    0
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
        "yx-internal {VERSION}\n\
         Runs inside a yxenv environment.\n\n\
         Layer entry:\n\
           yx env shell\n\
           yx env exec -- <command> [args...]\n\
           yx kas shell [kas.yml]\n\
           yx kas exec -- <command> [args...]\n\n\
         Workflow shortcuts:\n\
           yx kas dump [kas.yml]\n\
           yx kas checkout [kas.yml]\n\
           yx kas build [kas.yml]\n\
           yx bitbake <args...>\n\
           yx build [target]\n\
           yx devshell <recipe>\n\
           yx manifest [kas.yml]\n\
           yx doctor\n\n\
         Use --dry-run to print the real command without executing it."
    );
}
