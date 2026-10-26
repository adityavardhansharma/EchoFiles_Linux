//! The `ef` command line, as a library: the `ef` binary runs it in-process, and the
//! running EchoFiles app runs it for `ef` over a socket ([`server`]), with settings,
//! index maps, ignore rules and threads already loaded.
//!
//! Nothing here touches process-global state: the working directory, output streams and
//! the environment variables that matter come in a [`Ctx`].

macro_rules! outln {
    ($ctx:expr) => {{ let _ = $ctx.write_out(b"\n"); }};
    ($ctx:expr, $($t:tt)*) => {{ let _ = $ctx.write_out(format!("{}\n", format_args!($($t)*)).as_bytes()); }};
}

macro_rules! out {
    ($ctx:expr, $($t:tt)*) => {{ let _ = $ctx.write_out(format!($($t)*).as_bytes()); }};
}

macro_rules! errln {
    ($ctx:expr, $($t:tt)*) => {{ $ctx.write_err(format!("{}\n", format_args!($($t)*)).as_bytes()); }};
}

pub mod agent;
pub mod args;
pub mod exec;
pub mod find;
pub mod output;
pub mod phone;
pub mod query;
pub mod server;
pub mod tools;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::config::{self, Settings};
use args::Flavor;

/// An output stream shared by the threads of a run.
pub type Stream = Arc<Mutex<Box<dyn Write + Send>>>;

pub fn stream(w: impl Write + Send + 'static) -> Stream {
    Arc::new(Mutex::new(Box::new(w)))
}

/// Where a run happens and where its output goes.
pub struct Ctx {
    pub cwd: PathBuf,
    pub out: Stream,
    pub err: Stream,
    pub out_tty: bool,
    pub ls_colors: Option<String>,
    pub no_color: bool,
    pub settings: Arc<Settings>,
    /// Open index files, shared between runs in the app; opened on first use otherwise.
    pub indexes: Arc<find::Indexes>,
    /// Running inside the app: never exit or skip cleanup.
    pub served: bool,
    /// Whether the app (which keeps the index current) is running.
    pub app_running: bool,
}

impl Ctx {
    /// `p` (with `~` expanded) made absolute against this run's working directory.
    pub fn abs(&self, p: &str) -> PathBuf {
        let e = config::expand(p);
        if e.is_absolute() { e } else { self.cwd.join(e) }
    }

    pub fn write_out(&self, b: &[u8]) -> std::io::Result<()> {
        let mut o = self.out.lock().unwrap();
        o.write_all(b)?;
        o.flush()
    }

    pub fn write_err(&self, b: &[u8]) {
        let mut e = self.err.lock().unwrap();
        let _ = e.write_all(b);
        let _ = e.flush();
    }
}

pub const OFF: &str = "EchoFiles commands are turned off. Turn them on in EchoFiles → Settings → AI agents.";

pub fn help(ctx: &Ctx, flavor: Flavor) {
    match flavor {
        Flavor::Fd => outln!(ctx, "{}", include_str!("help_fd.txt")),
        Flavor::Ef => outln!(ctx, "{}", include_str!("help.txt")),
    }
}

/// Commands that must run in the caller's own process: they run programs (which need the
/// caller's environment), read stdin, or wait.
pub fn runs_locally(argv: &[String]) -> bool {
    let cmd = argv.first().map(String::as_str).unwrap_or("");
    if matches!(cmd, "batch" | "wait" | "index") {
        return true;
    }
    // Searches that run a command (-x, -X, and -l, which runs `ls`).
    let (flavor, rest) = match cmd {
        "fd" => (Flavor::Fd, &argv[1..]),
        "find" | "f" => (Flavor::Ef, &argv[1..]),
        "why" | "map" | "tree" | "recent" | "related" | "mark" | "changed" | "dupes" | "status" | "help" | "version" | "phone" => return false,
        _ => (Flavor::Ef, argv),
    };
    args::parse(flavor, rest).is_ok_and(|a| a.exec.is_some() || a.list_details)
}

/// Run one `ef` command line. Returns the exit code.
pub fn run(argv: &[String], ctx: &Ctx) -> u8 {
    if !ctx.settings.agents.cli {
        errln!(ctx, "{OFF}");
        return 3;
    }
    let cmd = argv.first().cloned().unwrap_or_default();
    let rest: &[String] = if argv.is_empty() { &[] } else { &argv[1..] };
    match cmd.as_str() {
        "find" | "f" => find::main(Flavor::Ef, rest, ctx),
        "fd" => find::main(Flavor::Fd, rest, ctx),
        "why" => tools::why(rest, ctx),
        "map" | "tree" => tools::map(rest, ctx),
        "recent" => tools::recent(rest, ctx),
        "related" => tools::related(rest, ctx),
        "wait" => tools::wait(rest, ctx),
        "mark" => tools::mark(rest, ctx),
        "changed" => tools::changed(rest, ctx),
        "batch" => tools::batch(rest, ctx),
        "dupes" => tools::dupes(rest, ctx),
        "status" => tools::status(ctx),
        "index" => tools::index(rest, ctx),
        "phone" => phone::main(rest, ctx),
        "" | "help" | "-h" | "--help" => {
            help(ctx, Flavor::Ef);
            0
        }
        "-V" | "--version" | "version" => {
            outln!(ctx, "ef {}", env!("CARGO_PKG_VERSION"));
            0
        }
        // `ef report` is `ef find report`.
        _ => find::main(Flavor::Ef, argv, ctx),
    }
}

/// The open index files for this process: in the app, shared by its search box and by
/// `ef` requests, so each index is mapped and validated once.
pub fn shared_indexes() -> Arc<find::Indexes> {
    static SHARED: std::sync::OnceLock<Arc<find::Indexes>> = std::sync::OnceLock::new();
    SHARED.get_or_init(Default::default).clone()
}

/// Whether `path` is under `root` (both absolute).
pub fn inside(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}
