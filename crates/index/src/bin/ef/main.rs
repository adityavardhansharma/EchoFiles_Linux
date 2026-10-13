//! `ef` — EchoFiles from the terminal and for AI agents. See `ef help`.
//!
//! `ef fd …` (or `ef` installed under the name `fd`) takes fd's options and defaults.
//! `EF_TIMING=1` prints timings to stderr.

use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use ef_index::cli::{self, Ctx};
use ef_index::config::Settings;

fn main() -> ExitCode {
    let mut argv: Vec<String> = Vec::new();
    for a in std::env::args_os().skip(1) {
        match a.into_string() {
            Ok(s) => argv.push(s),
            Err(_) => {
                eprintln!("ef: arguments must be valid UTF-8");
                return ExitCode::from(2);
            }
        }
    }
    if std::env::args_os().next().is_some_and(|a| Path::new(&a).file_name().is_some_and(|n| n == "fd")) {
        argv.insert(0, "fd".into());
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| "/".into());
    let out_tty = std::io::stdout().is_terminal();
    let settings = match Settings::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let ctx = Ctx {
        cwd,
        out: cli::stream(std::io::stdout()),
        err: cli::stream(std::io::stderr()),
        out_tty,
        ls_colors: std::env::var("LS_COLORS").ok(),
        no_color: std::env::var_os("NO_COLOR").is_some(),
        settings: Arc::new(settings),
        indexes: Default::default(),
        served: false,
        app_running: std::os::unix::net::UnixStream::connect(ef_index::config::socket_path()).is_ok(),
    };
    let code = cli::run(&argv, &ctx);
    let _ = ctx.out.lock().map(|mut o| o.flush());
    ExitCode::from(code)
}
