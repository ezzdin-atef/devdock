//! Where a command serves: a saved port per command (auto-picked once, or fixed),
//! how that port is handed to the dev server, and the project's `*.localhost` name.
//!
//! Browsers resolve every `*.localhost` name to the loopback address, so a project
//! can be opened at `http://shop.localhost:4821` with no hosts-file edit and no admin
//! rights. Each name is its own origin, so cookies and storage stay per project.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::spec::{CommandSource, PackageManager, RunCommand};
use crate::paths;

/// How a command picks its port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortMode {
    /// DevDock doesn't touch the port; the server uses its own default.
    Default,
    /// DevDock picks a free port once, saves it, and reuses it.
    Auto,
    /// The user's port.
    Fixed,
}

/// Saved per command inside `Project::run_prefs`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RunPrefs {
    /// `None` = follow Settings ("auto-port for commands" on or off).
    pub port_mode: Option<PortMode>,
    /// The saved port: picked by DevDock in auto mode, typed by the user in fixed.
    pub port: Option<u16>,
    /// What is appended to the command to set the port, with `{port}` in it.
    /// `None` = DevDock decides from the script; `Some("")` = only set `PORT`.
    pub port_args: Option<String>,
}

impl RunPrefs {
    pub fn mode(&self, auto_by_default: bool) -> PortMode {
        self.port_mode.unwrap_or(if auto_by_default {
            PortMode::Auto
        } else {
            PortMode::Default
        })
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Auto ports come from here: above the usual dev defaults (3000, 5173, 8080...)
/// and below the Windows ephemeral range (49152+).
pub const AUTO_RANGE: std::ops::RangeInclusive<u16> = 4100..=9899;

/// Ports that some tool already claims by default; auto mode never picks them.
const WELL_KNOWN: [u16; 18] = [
    4200, 4321, 5000, 5001, 5173, 5174, 5432, 5500, 6006, 6379, 7000, 8000, 8008, 8080, 8081, 8443,
    8888, 9000,
];

/// A free port for `seed` (the run key), avoiding `taken` (listening now) and
/// `reserved` (saved for other commands). The same seed starts at the same spot, so
/// a command that loses its saved port tends to get the same new one back.
pub fn pick_port(seed: &str, taken: &HashSet<u16>, reserved: &HashSet<u16>) -> Option<u16> {
    let lo = *AUTO_RANGE.start();
    let span = u32::from(*AUTO_RANGE.end() - lo) + 1;
    let start = (paths::stable_hash(seed) % u64::from(span)) as u32;
    (0..span)
        .map(|i| lo + ((start + i) % span) as u16)
        .find(|p| !taken.contains(p) && !reserved.contains(p) && !WELL_KNOWN.contains(p))
}

/// Dev servers that ignore `PORT` but take a flag, keyed by the tool's name.
/// (tool, words that must follow it or "" for any, flag template)
const FLAGS: [(&str, &str, &str); 14] = [
    ("vite", "", "--port {port} --strictPort"),
    ("astro", "", "--port {port}"),
    ("ng", "serve", "--port {port}"),
    ("nuxt", "", "--port {port}"),
    ("nuxi", "", "--port {port}"),
    ("next", "", "--port {port}"),
    ("webpack", "serve", "--port {port}"),
    ("webpack-dev-server", "", "--port {port}"),
    ("vue-cli-service", "serve", "--port {port}"),
    ("storybook", "dev", "--port {port}"),
    ("docusaurus", "start", "--port {port}"),
    ("gatsby", "develop", "--port {port}"),
    ("wrangler", "dev", "--port {port}"),
    ("parcel", "", "--port {port}"),
];

/// The flag DevDock appends for a `package.json` script, if it recognises the tool.
/// Only for a single plain command: `a && b` or `concurrently ...` would get the
/// flag on the wrong program, so those rely on `PORT` alone.
pub fn detect_port_args(script: &str) -> Option<&'static str> {
    if script.contains(['&', '|', ';', '>', '<']) {
        return None;
    }
    let mut words = script.split_whitespace().peekable();
    // Skip `cross-env A=1`, `NODE_ENV=x`, `npx`, `pnpm exec`.
    while let Some(w) = words.peek() {
        if w.contains('=') || matches!(*w, "cross-env" | "npx" | "bunx" | "exec" | "dotenv") {
            words.next();
        } else if matches!(*w, "pnpm" | "yarn") {
            words.next();
            if words.peek() == Some(&"exec") {
                words.next();
            }
        } else {
            break;
        }
    }
    let tool = words.next()?;
    let next = words.next().unwrap_or("");
    FLAGS
        .iter()
        .find(|(t, sub, _)| *t == tool && (sub.is_empty() || *sub == next))
        .map(|(_, _, flag)| *flag)
}

/// The final command line and env for running `command` on `port`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    pub command_line: String,
    pub env: Vec<(String, String)>,
}

/// `PORT` is always set; a flag is added when the user gave one or DevDock knows
/// the tool. Script flags go through the package manager (`npm run dev -- --port`).
pub fn serve_on(command: &RunCommand, port: u16, prefs: &RunPrefs, pm: PackageManager) -> Served {
    let template = match (&prefs.port_args, &command.source) {
        (Some(own), _) => Some(own.trim().to_owned()),
        (None, CommandSource::Script { body }) => detect_port_args(body).map(str::to_owned),
        (None, _) => None,
    }
    .filter(|t| !t.is_empty());
    let mut line = command.command_line.clone();
    if let Some(t) = template {
        let args = t.replace("{port}", &port.to_string());
        let through_npm =
            matches!(command.source, CommandSource::Script { .. }) && pm == PackageManager::Npm;
        line = if through_npm {
            format!("{line} -- {args}")
        } else {
            format!("{line} {args}")
        };
    }
    Served {
        command_line: line,
        env: vec![("PORT".into(), port.to_string())],
    }
}

/// Checks a user's port flag: it must mention `{port}` and can't chain commands.
pub fn validate_port_args(args: &str) -> Result<String, String> {
    let args = args.trim();
    if args.is_empty() {
        return Ok(String::new());
    }
    if !args.contains("{port}") {
        return Err("Put {port} where the port number goes, for example --port {port}.".into());
    }
    if args.contains(['&', '|', ';', '>', '<', '^', '"', '\n']) {
        return Err("The port flag can't contain & | ; < > ^ or quotes.".into());
    }
    Ok(args.to_owned())
}

pub const DOMAIN_SUFFIX: &str = ".localhost";

/// `My Shop_v2` -> `my-shop-v2`. Empty when nothing usable is left.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    out.chars()
        .take(63)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned()
}

/// The name a project gets unless the user set one: `<slug>.localhost`.
pub fn default_domain(project_name: &str) -> String {
    let s = slug(project_name);
    format!("{}{DOMAIN_SUFFIX}", if s.is_empty() { "app" } else { &s })
}

/// Endings a project address may use. `.localhost` works everywhere with no setup;
/// `.test` (reserved for testing, never a real site) needs a hosts-file entry.
pub const SUFFIXES: [&str; 2] = [DOMAIN_SUFFIX, ".test"];

/// Cleans a typed domain: `Shop` -> `shop.localhost`, `api.shop.localhost` and
/// `shop.test` stay. Any other ending gets `.localhost` added: real endings like
/// `.dev` would send the browser elsewhere, and `.local` belongs to the network.
pub fn normalize_domain(input: &str) -> Result<String, String> {
    let d = input.trim().trim_end_matches('.').to_ascii_lowercase();
    let d = d
        .strip_prefix("http://")
        .or_else(|| d.strip_prefix("https://"))
        .unwrap_or(&d)
        .trim_end_matches('/')
        .to_owned();
    if d.is_empty() {
        return Err("Type a name, for example shop.".into());
    }
    let suffix = SUFFIXES.iter().find(|s| d.ends_with(*s) || d == s[1..]);
    // Anything else gets `.localhost`, so `shop.dev` can never reach the real web.
    let full = match suffix {
        Some(_) => d,
        None => format!("{d}{DOMAIN_SUFFIX}"),
    };
    let suffix = SUFFIXES
        .iter()
        .find(|s| full.ends_with(*s))
        .copied()
        .unwrap_or(DOMAIN_SUFFIX);
    let host = full.strip_suffix(suffix).unwrap_or("");
    let label_ok = |l: &str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    if host.is_empty() || !host.split('.').all(label_ok) {
        return Err(format!(
            "{full} isn't a valid name. Use letters, digits and dashes, like shop or api.shop."
        ));
    }
    Ok(full)
}

/// A name DevDock may serve or issue a certificate for: a valid `*.localhost` or
/// `*.test` name (not bare `localhost`).
pub fn is_local_host(host: &str) -> bool {
    normalize_domain(host).is_ok_and(|d| d == host)
}

/// `*.test` names need a hosts-file entry; `*.localhost` ones don't.
pub fn needs_hosts_entry(host: &str) -> bool {
    host.ends_with(".test")
}

/// Only these URLs are opened from the app: local dev addresses, http or https,
/// with or without a port.
pub fn is_local_url(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
    else {
        return false;
    };
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (host, port_ok) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().is_ok_and(|p| p > 0)),
        None => (host_port, true),
    };
    let host_ok = host == "localhost" || host == "127.0.0.1" || is_local_host(host);
    host_ok && port_ok
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::run::CommandId;

    fn script(body: &str) -> RunCommand {
        RunCommand {
            id: CommandId("script:dev".into()),
            name: "dev".into(),
            command_line: "npm run dev".into(),
            cwd: PathBuf::from("."),
            source: CommandSource::Script { body: body.into() },
        }
    }

    #[test]
    fn picks_skip_taken_reserved_and_well_known() {
        let p = pick_port("a/b", &HashSet::new(), &HashSet::new()).unwrap();
        assert!(AUTO_RANGE.contains(&p));
        assert_eq!(pick_port("a/b", &HashSet::new(), &HashSet::new()), Some(p));
        let taken: HashSet<u16> = [p].into();
        let q = pick_port("a/b", &taken, &HashSet::new()).unwrap();
        assert_ne!(q, p);
        let all: HashSet<u16> = AUTO_RANGE.collect();
        assert_eq!(pick_port("a/b", &all, &HashSet::new()), None);
        for seed in 0..500 {
            let p = pick_port(&seed.to_string(), &HashSet::new(), &HashSet::new()).unwrap();
            assert!(!WELL_KNOWN.contains(&p));
        }
    }

    #[test]
    fn known_tools_get_a_flag() {
        assert_eq!(detect_port_args("vite"), Some("--port {port} --strictPort"));
        assert_eq!(
            detect_port_args("vite dev --host"),
            Some("--port {port} --strictPort")
        );
        assert_eq!(
            detect_port_args("cross-env NODE_ENV=dev astro dev"),
            Some("--port {port}")
        );
        assert_eq!(detect_port_args("ng serve"), Some("--port {port}"));
        assert_eq!(detect_port_args("ng build"), None);
        assert_eq!(detect_port_args("next dev"), Some("--port {port}"));
        assert_eq!(detect_port_args("node server.js"), None);
        assert_eq!(detect_port_args("vite build && vite preview"), None);
        assert_eq!(detect_port_args("concurrently \"vite\" \"tsc -w\""), None);
    }

    #[test]
    fn npm_scripts_pass_flags_after_double_dash() {
        let s = serve_on(
            &script("vite"),
            4500,
            &RunPrefs::default(),
            PackageManager::Npm,
        );
        assert_eq!(s.command_line, "npm run dev -- --port 4500 --strictPort");
        assert_eq!(s.env, vec![("PORT".into(), "4500".into())]);
        let mut c = script("vite");
        c.command_line = "pnpm run dev".into();
        let s = serve_on(&c, 4500, &RunPrefs::default(), PackageManager::Pnpm);
        assert_eq!(s.command_line, "pnpm run dev --port 4500 --strictPort");
    }

    #[test]
    fn unknown_or_env_only_leaves_the_line_alone() {
        let s = serve_on(
            &script("node ."),
            4500,
            &RunPrefs::default(),
            PackageManager::Npm,
        );
        assert_eq!(s.command_line, "npm run dev");
        let env_only = RunPrefs {
            port_args: Some(String::new()),
            ..Default::default()
        };
        let s = serve_on(&script("vite"), 4500, &env_only, PackageManager::Npm);
        assert_eq!(s.command_line, "npm run dev");
    }

    #[test]
    fn custom_commands_use_the_users_flag() {
        let c = RunCommand {
            source: CommandSource::Custom,
            command_line: "python manage.py runserver".into(),
            ..script("")
        };
        let prefs = RunPrefs {
            port_args: Some("127.0.0.1:{port}".into()),
            ..Default::default()
        };
        let s = serve_on(&c, 4500, &prefs, PackageManager::Npm);
        assert_eq!(s.command_line, "python manage.py runserver 127.0.0.1:4500");
    }

    #[test]
    fn port_args_validation() {
        assert_eq!(validate_port_args("  ").unwrap(), "");
        assert!(validate_port_args("--port").is_err());
        assert!(validate_port_args("--port {port} & calc").is_err());
        assert_eq!(validate_port_args(" -p {port} ").unwrap(), "-p {port}");
    }

    #[test]
    fn domains() {
        assert_eq!(default_domain("My Shop_v2"), "my-shop-v2.localhost");
        assert_eq!(default_domain("___"), "app.localhost");
        assert_eq!(normalize_domain("Shop").unwrap(), "shop.localhost");
        assert_eq!(
            normalize_domain("api.shop.localhost").unwrap(),
            "api.shop.localhost"
        );
        assert_eq!(
            normalize_domain("http://shop.localhost/").unwrap(),
            "shop.localhost"
        );
        assert!(normalize_domain("").is_err());
        assert!(normalize_domain("localhost").is_err());
        assert!(normalize_domain("bad_name").is_err());
        assert!(normalize_domain("-x").is_err());
        assert_eq!(normalize_domain("Shop.Test").unwrap(), "shop.test");
        assert_eq!(normalize_domain("shop.dev").unwrap(), "shop.dev.localhost");
        assert!(normalize_domain("test").is_err());
        assert!(is_local_host("api.shop.test"));
        assert!(!is_local_host("localhost"));
    }

    #[test]
    fn only_local_urls_open() {
        assert!(is_local_url("http://shop.localhost:4500"));
        assert!(is_local_url("http://localhost:3000/"));
        assert!(!is_local_url("http://evil.com:80"));
        assert!(is_local_url("https://shop.localhost"));
        assert!(is_local_url("http://shop.test"));
        assert!(!is_local_url("http://shop.localhost:0"));
        assert!(!is_local_url("https://shop.dev"));
        assert!(!is_local_url("http://a&b.localhost:1"));
    }
}
