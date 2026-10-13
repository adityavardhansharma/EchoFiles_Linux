//! `-x`/`--exec` (one command per result, in parallel) and `-X`/`--exec-batch` (one
//! command for many results), with fd's placeholders.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::args::Exec;
use super::output::{expand, has_placeholder};

fn command(cmd: &[String], paths: &[&[u8]]) -> Command {
    let mut c = Command::new(&cmd[0]);
    let any = cmd.iter().any(|a| has_placeholder(a));
    for arg in &cmd[1..] {
        if has_placeholder(arg) {
            for p in paths {
                c.arg(OsStr::from_bytes(&expand(arg, p)));
            }
        } else {
            c.arg(arg);
        }
    }
    if !any {
        for p in paths {
            c.arg(OsStr::from_bytes(p));
        }
    }
    c
}

/// `print0`: like fd, end each command's output with a NUL.
pub fn run(x: &Exec, paths: &[Vec<u8>], threads: Option<usize>, batch_size: Option<usize>, print0: bool, ctx: &super::Ctx) -> u8 {
    if x.batch {
        return run_batch(x, paths, batch_size, ctx);
    }
    // `{}` in the command name itself is expanded too.
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
                    let mut cmd_v = x.cmd.clone();
                    if has_placeholder(&cmd_v[0]) {
                        cmd_v[0] = String::from_utf8_lossy(&expand(&cmd_v[0], p)).into_owned();
                    }
                    let mut c = command(&cmd_v, &[p.as_slice()]);
                    if jobs == 1 {
                        let st = c.status();
                        if print0 {
                            let _ = ctx.write_out(b"\0");
                        }
                        match st {
                            Ok(st) if st.success() => {}
                            Ok(_) => failed.store(true, Ordering::Relaxed),
                            Err(e) => {
                                errln!(ctx, "[ef error]: Command not found: {} ({e})", cmd_v[0]);
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
                            errln!(ctx, "[ef error]: Command not found: {} ({e})", cmd_v[0]);
                            failed.store(true, Ordering::Relaxed);
                        }
                    }
                }
            });
        }
    });
    if failed.load(Ordering::Relaxed) { 1 } else { 0 }
}

fn run_batch(x: &Exec, paths: &[Vec<u8>], batch_size: Option<usize>, ctx: &super::Ctx) -> u8 {
    if paths.is_empty() {
        return 0;
    }
    // Keep each command line well under ARG_MAX.
    const MAX_BYTES: usize = 1 << 20;
    let limit = batch_size.filter(|&n| n > 0).unwrap_or(usize::MAX);
    let mut failed = false;
    let mut start = 0;
    while start < paths.len() {
        let mut end = start;
        let mut bytes = 0;
        while end < paths.len() && end - start < limit && (end == start || bytes + paths[end].len() + 1 < MAX_BYTES) {
            bytes += paths[end].len() + 1;
            end += 1;
        }
        let batch: Vec<&[u8]> = paths[start..end].iter().map(Vec::as_slice).collect();
        match command(&x.cmd, &batch).status() {
            Ok(st) if st.success() => {}
            Ok(_) => failed = true,
            Err(e) => {
                errln!(ctx, "[ef error]: Command not found: {} ({e})", x.cmd[0]);
                return 1;
            }
        }
        start = end;
    }
    if failed { 1 } else { 0 }
}
