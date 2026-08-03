use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub const YX_PROTOCOL: u32 = 1;

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub profile: String,
    pub backend: String,
    pub yxenv_ref: String,
    pub image: Option<String>,
    pub kas_default: String,
    pub downloads: String,
    pub sstate: String,
    pub build_dir: String,
    pub default_target: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        let root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            root,
            profile: "yocto-scarthgap-kas52".to_string(),
            backend: "nix-develop".to_string(),
            yxenv_ref: "github:why-ex/yx-env".to_string(),
            image: None,
            kas_default: "kas/project.yml".to_string(),
            downloads: ".yx/downloads".to_string(),
            sstate: ".yx/sstate".to_string(),
            build_dir: "build".to_string(),
            default_target: "core-image-minimal".to_string(),
        }
    }
}

impl ProjectConfig {
    pub fn discover() -> Self {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let Some(config_path) = find_upwards(&cwd, ".yx/project.toml") else {
            let mut cfg = Self::default();
            cfg.root = cwd;
            return cfg;
        };

        let root = config_path
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| cwd.clone());

        let text = fs::read_to_string(&config_path).unwrap_or_default();
        let mut cfg = Self { root, ..Self::default() };
        cfg.apply_toml_like(&text);
        cfg
    }

    fn apply_toml_like(&mut self, text: &str) {
        let mut section = String::new();
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line.trim_matches(&['[', ']'][..]).trim().to_string();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue; };
            let key = key.trim();
            let value = unquote(value.trim());
            match (section.as_str(), key) {
                ("env", "profile") => self.profile = value,
                ("env", "backend") => self.backend = value,
                ("env", "yxenv") | ("env", "yxenv_ref") => self.yxenv_ref = value,
                ("env", "image") => self.image = Some(value),
                ("kas", "default") => self.kas_default = value,
                ("paths", "downloads") => self.downloads = value,
                ("paths", "sstate") => self.sstate = value,
                ("paths", "build") | ("paths", "build_dir") => self.build_dir = value,
                ("build", "target") | ("build", "default_target") => self.default_target = value,
                _ => {}
            }
        }
    }

    pub fn kas_file(&self, arg: Option<&String>) -> String {
        arg.cloned().unwrap_or_else(|| self.kas_default.clone())
    }
}

#[derive(Debug, Clone)]
pub struct EnvState {
    pub inside: bool,
    pub profile: Option<String>,
    pub version: Option<String>,
    pub backend: Option<String>,
    pub layer: Option<String>,
}

impl EnvState {
    pub fn detect() -> Self {
        Self {
            inside: env::var("YXENV").ok().as_deref() == Some("1"),
            profile: env::var("YXENV_PROFILE").ok(),
            version: env::var("YXENV_VERSION").ok(),
            backend: env::var("YXENV_BACKEND").ok(),
            layer: env::var("YX_LAYER").ok(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CmdSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub envs: Vec<(String, String)>,
}

impl CmdSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self { program: program.into(), args: Vec::new(), cwd: None, envs: Vec::new() }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.push((key.into(), value.into()));
        self
    }

    pub fn printable(&self) -> String {
        let mut parts = Vec::new();
        for (k, v) in &self.envs {
            parts.push(format!("{}={}", shell_quote(k), shell_quote(v)));
        }
        parts.push(shell_quote(&self.program));
        parts.extend(self.args.iter().map(|a| shell_quote(a)));
        parts.join(" ")
    }

    pub fn status(&self) -> io::Result<ExitStatus> {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }
        for (k, v) in &self.envs {
            cmd.env(k, v);
        }
        cmd.status()
    }
}

pub fn run_or_print(cmd: &CmdSpec, dry_run: bool) -> i32 {
    eprintln!("+ {}", cmd.printable());
    if dry_run {
        return 0;
    }
    match cmd.status() {
        Ok(status) => status.code().unwrap_or(128),
        Err(err) => {
            eprintln!("yx: failed to execute {}: {}", cmd.program, err);
            127
        }
    }
}

pub fn split_after_double_dash(args: &[String]) -> Vec<String> {
    match args.iter().position(|a| a == "--") {
        Some(pos) => args[pos + 1..].to_vec(),
        None => args.to_vec(),
    }
}

pub fn join_shell_words(args: &[String]) -> String {
    args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")
}

pub fn shell_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "@%_+=:,./-".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

pub fn command_exists(name: &str) -> bool {
    if name.contains('/') {
        return Path::new(name).is_file();
    }
    let Some(paths) = env::var_os("PATH") else { return false; };
    env::split_paths(&paths).any(|p| p.join(name).is_file())
}

pub fn current_uid_gid() -> (String, String) {
    fn run_id(flag: &str) -> Option<String> {
        let out = Command::new("id").arg(flag).output().ok()?;
        if !out.status.success() {
            return None;
        }
        String::from_utf8(out.stdout).ok().map(|s| s.trim().to_string())
    }
    (
        run_id("-u").unwrap_or_else(|| "1000".to_string()),
        run_id("-g").unwrap_or_else(|| "1000".to_string()),
    )
}

pub fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

pub fn os_args_without_program() -> Vec<String> {
    env::args_os()
        .skip(1)
        .map(os_to_string_lossy)
        .collect()
}

fn os_to_string_lossy(s: OsString) -> String {
    s.to_string_lossy().into_owned()
}

fn find_upwards(start: &Path, rel: &str) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}
