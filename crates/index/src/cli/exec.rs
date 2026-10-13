//! `-x`/`--exec` (one command per result, in parallel) and `-X`/`--exec-batch` (one
//! command for many results), with fd's templates and fd's rules for where the paths go.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::args::Exec;
use super::output::Template;

/// The command's templates, with `{}` appended when none has a placeholder.
fn templates(cmd: &[String]) -> Vec<Template> {
    let mut t: Vec<Template> = cmd.iter().map(|a| Template::parse(a)).collect();
    if !t.iter().any(Template::has_tokens) {
        t.push(Template::placeholder());
    }
    t
}

/// `print0`: like fd, end each command's output with a NUL. `sep` replaces `/` in the
/// paths given to the command (fd's `--path-separator`).
pub fn run(x: &Exec, paths: &[Vec<u8>], sep: Option<&str>, threads: Option<usize>, batch_size: Option<usize>, print0: bool, ctx: &super::Ctx) -> u8 {
    let t = templates(&x.cmd);
    if x.batch {
        if t[0].has_tokens() {
            ctx.write_err(b"[ef error]: First argument of --exec-batch must be a fixed executable, not a placeholder\n");
            return 1;
        }
        return run_batch(&t, paths, sep, batch_size, ctx);
    }
    let jobs = threads.unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get())).max(1);
    let failed = AtomicBool::new(false);
    let next = AtomicUsize::new(0);
    let out_lock = Mutex::new(());
    std::thread::scope(|s| {
        for _ in 0..jobs.min(paths.len().max(1)) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(p) = paths.get(i) else { break };
                    let args: Vec<Vec<u8>> = t.iter().map(|a| a.generate(p, sep)).collect();
                    let mut c = Command::new(OsStr::from_bytes(&args[0]));
                    c.args(args[1..].iter().map(|a| OsStr::from_bytes(a)));
                    let name = String::from_utf8_lossy(&args[0]).into_owned();
                    if jobs == 1 {
                        let st = c.status();
                        if print0 {
                            let _ = ctx.write_out(b"\0");
                        }
                        match st {
                            Ok(st) if st.success() => {}
                            Ok(_) => failed.store(true, Ordering::Relaxed),
                            Err(e) => {
                                ctx.write_err(format!("[ef error]: Command not found: {name} ({e})\n").as_bytes());
                                failed.store(true, Ordering::Relaxed);
                            }
                        }
                        continue;
                    }
                    // Buffer each command's output so parallel runs don't interleave lines.
                    match c.stdin(Stdio::null()).output() {
                        Ok(o) => {
                            let _g = out_lock.lock().unwrap();
                            let _ = ctx.write_out(&o.stdout);
                            if print0 {
                                let _ = ctx.write_out(b"\0");
                            }
                            ctx.write_err(&o.stderr);
                            if !o.status.success() {
                                failed.store(true, Ordering::Relaxed);
                            }
                        }
                        Err(e) => {
                            ctx.write_err(format!("[ef error]: Command not found: {name} ({e})\n").as_bytes());
                            failed.store(true, Ordering::Relaxed);
                        }
                    }
                }
            });
        }
    });
    u8::from(failed.load(Ordering::Relaxed))
}

/// fd's batch layout: the fixed arguments before the (last) argument with a placeholder,
/// that argument once per path, then the fixed arguments after it.
fn run_batch(t: &[Template], paths: &[Vec<u8>], sep: Option<&str>, batch_size: Option<usize>, ctx: &super::Ctx) -> u8 {
    if paths.is_empty() {
        return 0;
    }
    let (mut pre, mut post): (Vec<Vec<u8>>, Vec<Vec<u8>>) = (Vec::new(), Vec::new());
    let mut path_arg: Option<&Template> = None;
    for a in t {
        if a.has_tokens() {
            path_arg = Some(a);
        } else if path_arg.is_none() {
            pre.push(a.generate(b"", None));
        } else {
            post.push(a.generate(b"", None));
        }
    }
    let path_arg = path_arg.expect("a placeholder was appended");
    // Keep each command line well under ARG_MAX.
    const MAX_BYTES: usize = 1 << 20;
    let limit = batch_size.filter(|&n| n > 0).unwrap_or(usize::MAX);
    let fixed: usize = pre.iter().chain(&post).map(|a| a.len() + 1).sum();
    let mut failed = false;
    let mut start = 0;
    while start < paths.len() {
        let mut args: Vec<Vec<u8>> = Vec::new();
        let mut bytes = fixed;
        let mut end = start;
        while end < paths.len() && end - start < limit {
            let a = path_arg.generate(&paths[end], sep);
            if end > start && bytes + a.len() + 1 >= MAX_BYTES {
                break;
            }
            bytes += a.len() + 1;
            args.push(a);
            end += 1;
        }
        let mut c = Command::new(OsStr::from_bytes(&pre[0]));
        c.args(pre[1..].iter().chain(&args).chain(&post).map(|a| OsStr::from_bytes(a)));
        match c.status() {
            Ok(st) if st.success() => {}
            Ok(_) => failed = true,
            Err(e) => {
                ctx.write_err(format!("[ef error]: Command not found: {} ({e})\n", String::from_utf8_lossy(&pre[0])).as_bytes());
                return 1;
            }
        }
        start = end;
    }
    u8::from(failed)
}
