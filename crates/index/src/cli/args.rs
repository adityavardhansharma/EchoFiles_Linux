//! Command-line parsing for `ef find` and `ef fd`: every fd option (same names, same
//! clustering of short flags, `--flag=value`, `-x … ;`), plus EchoFiles' own.

use std::path::PathBuf;

use crate::filter::Types;
use crate::pattern::{Case, Syntax};
use crate::units::SizeFilter;

/// `ef find` (EchoFiles defaults) or `ef fd` (fd's defaults, exactly).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    Ef,
    Fd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum When {
    #[default]
    Auto,
    Always,
    Never,
}

impl When {
    fn parse(v: &str) -> Result<When, String> {
        match v {
            "auto" => Ok(When::Auto),
            "always" => Ok(When::Always),
            "never" => Ok(When::Never),
            _ => Err(format!("expected auto, always or never, not '{v}'")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Exec {
    /// `-X`: one command for all results (in batches).
    pub batch: bool,
    pub cmd: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Rank,
    Path,
    Name,
    Size,
    Modified,
    None,
}

#[derive(Clone, Debug)]
pub struct FindArgs {
    pub flavor: Flavor,
    pub pattern: Option<String>,
    pub and: Vec<String>,
    pub paths: Vec<String>,
    pub syntax: Option<Syntax>,
    pub case: Case,
    pub full_path: bool,
    pub hidden: bool,
    pub no_ignore: bool,
    pub no_ignore_vcs: bool,
    pub no_require_git: bool,
    pub no_ignore_parent: bool,
    pub no_global_ignore_file: bool,
    pub ignore_files: Vec<PathBuf>,
    pub ignore_contain: Vec<String>,
    pub excludes: Vec<String>,
    pub types: Types,
    pub exts: Vec<String>,
    pub sizes: Vec<SizeFilter>,
    pub newer: Option<String>,
    pub older: Option<String>,
    pub owner: Option<String>,
    pub max_depth: Option<u32>,
    pub min_depth: Option<u32>,
    pub prune: bool,
    pub follow: bool,
    pub one_fs: bool,
    pub absolute: bool,
    pub relative: bool,
    pub print0: bool,
    pub list_details: bool,
    pub format: Option<String>,
    pub color: When,
    pub hyperlink: When,
    pub path_separator: Option<String>,
    pub strip_cwd_prefix: Option<When>,
    pub exec: Option<Exec>,
    pub batch_size: Option<usize>,
    pub threads: Option<usize>,
    pub quiet: bool,
    pub max_results: Option<usize>,
    pub show_errors: bool,
    pub base_dir: Option<PathBuf>,
    // EchoFiles additions
    pub count: bool,
    pub json: bool,
    pub budget: Option<usize>,
    pub summary: bool,
    pub live: bool,
    pub indexed: bool,
    pub sort: Option<Sort>,
    pub project: bool,
    pub here: bool,
    pub verbose: bool,
    pub details: bool,
    /// Also ask Google Drive itself, not just the Drive index (needs EchoFiles running).
    pub drive_live: bool,
    /// Leave out what Settings → Search excludes from the index, even where read live.
    pub respect_settings: bool,
    pub help: bool,
    pub version: bool,
}

impl FindArgs {
    fn new(flavor: Flavor) -> FindArgs {
        FindArgs {
            flavor,
            pattern: None,
            and: Vec::new(),
            paths: Vec::new(),
            syntax: None,
            case: Case::Smart,
            full_path: false,
            hidden: false,
            no_ignore: false,
            no_ignore_vcs: false,
            no_require_git: false,
            no_ignore_parent: false,
            no_global_ignore_file: false,
            ignore_files: Vec::new(),
            ignore_contain: Vec::new(),
            excludes: Vec::new(),
            types: Types::default(),
            exts: Vec::new(),
            sizes: Vec::new(),
            newer: None,
            older: None,
            owner: None,
            max_depth: None,
            min_depth: None,
            prune: false,
            follow: false,
            one_fs: false,
            absolute: false,
            relative: false,
            print0: false,
            list_details: false,
            format: None,
            color: When::Auto,
            hyperlink: When::Never,
            path_separator: None,
            strip_cwd_prefix: None,
            exec: None,
            batch_size: None,
            threads: None,
            quiet: false,
            max_results: None,
            show_errors: false,
            base_dir: None,
            count: false,
            json: false,
            budget: None,
            summary: false,
            live: false,
            indexed: false,
            sort: None,
            project: false,
            here: false,
            verbose: false,
            details: false,
            drive_live: false,
            respect_settings: false,
            help: false,
            version: false,
        }
    }
}

/// Long flags that take a value, with their short forms.
const VALUE: &[(&str, Option<char>)] = &[
    ("extension", Some('e')),
    ("ext", None),
    ("type", Some('t')),
    ("size", Some('S')),
    ("max-depth", Some('d')),
    ("maxdepth", None),
    ("min-depth", None),
    ("exact-depth", None),
    ("exclude", Some('E')),
    ("ignore-file", None),
    ("ignore-contain", None),
    ("owner", Some('o')),
    ("changed-within", None),
    ("change-newer-than", None),
    ("newer", None),
    ("changed-after", None),
    ("changed-before", None),
    ("change-older-than", None),
    ("older", None),
    ("color", Some('c')),
    ("threads", Some('j')),
    ("max-results", None),
    ("limit", Some('n')),
    ("batch-size", None),
    ("base-directory", Some('C')),
    ("path-separator", None),
    ("search-path", None),
    ("in", None),
    ("format", None),
    ("and", None),
    ("budget", None),
    ("sort", None),
];

/// Flags whose value is optional (`--hyperlink`, `--hyperlink=always`).
const OPTIONAL: &[&str] = &["hyperlink", "strip-cwd-prefix"];

fn short_value(c: char) -> Option<&'static str> {
    VALUE.iter().find(|(_, s)| *s == Some(c)).map(|(l, _)| *l)
}

fn parse_num<T: std::str::FromStr>(flag: &str, v: &str) -> Result<T, String> {
    v.parse().map_err(|_| format!("--{flag}: '{v}' is not a number"))
}

pub fn parse(flavor: Flavor, argv: &[String]) -> Result<FindArgs, String> {
    let mut a = FindArgs::new(flavor);
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    let mut only_positional = false;
    let mut exact_depth = None;
    // fd's pattern flags: --regex and --glob override each other (last wins); -F and
    // --exact are separate switches.
    let (mut glob, mut fixed, mut exact) = (None::<bool>, false, false);
    let (mut one, mut max_given, mut search_path, mut strip_given) = (false, false, false, false);
    while i < argv.len() {
        let arg = &argv[i];
        i += 1;
        if only_positional || arg == "-" || !arg.starts_with('-') {
            positional.push(arg.clone());
            continue;
        }
        if arg == "--" {
            only_positional = true;
            continue;
        }
        // Collect (name, inline value) pairs from `--long[=v]` or a `-abc` cluster.
        let mut flags: Vec<(String, Option<String>)> = Vec::new();
        if let Some(long) = arg.strip_prefix("--") {
            match long.split_once('=') {
                Some((n, v)) => flags.push((n.to_string(), Some(v.to_string()))),
                None => flags.push((long.to_string(), None)),
            }
        } else {
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut k = 0;
            while k < chars.len() {
                let c = chars[k];
                k += 1;
                if c == 'x' || c == 'X' {
                    flags.push((if c == 'x' { "exec".into() } else { "exec-batch".into() }, None));
                    if k < chars.len() {
                        return Err(format!("-{c} must come last in '{arg}'"));
                    }
                    break;
                }
                if let Some(long) = short_value(c) {
                    let rest: String = chars[k..].iter().collect();
                    flags.push((long.to_string(), (!rest.is_empty()).then_some(rest)));
                    break;
                }
                let long = match c {
                    'H' => "hidden",
                    'I' => "no-ignore",
                    'u' => "unrestricted",
                    's' => "case-sensitive",
                    'i' => "ignore-case",
                    'g' => "glob",
                    'r' => "regex",
                    'F' => "fixed-strings",
                    'a' => "absolute-path",
                    'l' => "list-details",
                    'L' => "follow",
                    'p' => "full-path",
                    '0' => "print0",
                    '1' => "1",
                    'q' => "quiet",
                    'h' => "help",
                    'V' => "version",
                    'v' => "verbose",
                    _ => return Err(format!("unknown option '-{c}'")),
                };
                flags.push((long.to_string(), None));
            }
        }
        for (name, inline) in flags {
            let takes = VALUE.iter().any(|(l, _)| *l == name);
            let value = if takes {
                match inline {
                    Some(v) => Some(v),
                    None => {
                        let v = argv.get(i).cloned().ok_or_else(|| format!("--{name} needs a value"))?;
                        i += 1;
                        Some(v)
                    }
                }
            } else if OPTIONAL.contains(&name.as_str()) {
                inline
            } else {
                if inline.is_some() {
                    return Err(format!("--{name} doesn't take a value"));
                }
                None
            };
            let v = value.unwrap_or_default();
            match name.as_str() {
                "extension" | "ext" => a.exts.extend(v.split(',').map(|e| e.trim_start_matches('.').to_string()).filter(|e| !e.is_empty())),
                "type" => {
                    for t in v.split(',') {
                        a.types.add(t)?;
                    }
                }
                "size" => a.sizes.push(SizeFilter::parse(&v).ok_or_else(|| format!("--size: can't read '{v}' (try +1m, -500k, 10KiB)"))?),
                "max-depth" | "maxdepth" => a.max_depth = Some(parse_num(&name, &v)?),
                "min-depth" => a.min_depth = Some(parse_num(&name, &v)?),
                "exact-depth" => exact_depth = Some(parse_num::<u32>(&name, &v)?),
                "exclude" => a.excludes.push(v),
                "ignore-file" => a.ignore_files.push(PathBuf::from(v)),
                "ignore-contain" => a.ignore_contain.push(v),
                "owner" => a.owner = Some(v),
                "changed-within" | "change-newer-than" | "newer" | "changed-after" => a.newer = Some(v),
                "changed-before" | "change-older-than" | "older" => a.older = Some(v),
                "color" => a.color = When::parse(&v)?,
                "hyperlink" => a.hyperlink = if v.is_empty() { When::Auto } else { When::parse(&v)? },
                "strip-cwd-prefix" => {
                    strip_given = true;
                    a.strip_cwd_prefix = Some(if v.is_empty() { When::Always } else { When::parse(&v)? });
                }
                "threads" => a.threads = Some(parse_num(&name, &v)?),
                "max-results" | "limit" => {
                    max_given = true;
                    a.max_results = Some(parse_num(&name, &v)?);
                }
                "batch-size" => a.batch_size = Some(parse_num(&name, &v)?),
                "base-directory" => a.base_dir = Some(PathBuf::from(v)),
                "path-separator" => a.path_separator = Some(v),
                "search-path" | "in" => {
                    search_path |= name == "search-path";
                    a.paths.push(v);
                }
                "format" => a.format = Some(v),
                "and" => a.and.push(v),
                "budget" => a.budget = Some(parse_num(&name, &v)?),
                "sort" => {
                    a.sort = Some(match v.as_str() {
                        "rank" | "best" => Sort::Rank,
                        "path" => Sort::Path,
                        "name" => Sort::Name,
                        "size" => Sort::Size,
                        "modified" | "mtime" | "time" | "recent" => Sort::Modified,
                        "none" => Sort::None,
                        _ => return Err(format!("--sort: use rank, path, name, size or modified, not '{v}'")),
                    })
                }
                "hidden" => a.hidden = true,
                "no-hidden" => a.hidden = false,
                "no-ignore" => a.no_ignore = true,
                "ignore" => a.no_ignore = false,
                "unrestricted" => {
                    a.no_ignore = true;
                    a.hidden = true;
                }
                "no-ignore-vcs" => a.no_ignore_vcs = true,
                "ignore-vcs" => a.no_ignore_vcs = false,
                "no-require-git" => a.no_require_git = true,
                "require-git" => a.no_require_git = false,
                "no-ignore-parent" => a.no_ignore_parent = true,
                "no-global-ignore-file" => a.no_global_ignore_file = true,
                "case-sensitive" => a.case = Case::Sensitive,
                "ignore-case" => a.case = Case::Insensitive,
                "glob" => glob = Some(true),
                "regex" => glob = Some(false),
                "fixed-strings" | "literal" => fixed = true,
                "exact" => exact = true,
                "words" => a.syntax = Some(Syntax::Words),
                "absolute-path" => a.absolute = true,
                "relative-path" | "relative" => {
                    a.relative = true;
                    a.absolute = false;
                }
                "list-details" => a.list_details = true,
                "follow" => a.follow = true,
                "no-follow" => a.follow = false,
                "full-path" => a.full_path = true,
                "print0" => a.print0 = true,
                "1" => {
                    one = true;
                    a.max_results = Some(1);
                }
                "quiet" | "has-results" => a.quiet = true,
                "show-errors" => a.show_errors = true,
                "one-file-system" | "mount" | "xdev" => a.one_fs = true,
                "prune" => a.prune = true,
                "help" => a.help = true,
                "version" => a.version = true,
                "exec" | "exec-batch" => {
                    let mut cmd = Vec::new();
                    while i < argv.len() && argv[i] != ";" {
                        cmd.push(argv[i].clone());
                        i += 1;
                    }
                    i += 1;
                    if cmd.is_empty() {
                        return Err(format!("--{name} needs a command"));
                    }
                    let batch = name == "exec-batch";
                    if flavor == Flavor::Fd
                        && let Some(prev) = &a.exec
                        && prev.batch != batch
                    {
                        return Err("the argument '--exec-batch <cmd>...' cannot be used with '--exec <cmd>...'".into());
                    }
                    a.exec = Some(Exec { batch, cmd });
                }
                // EchoFiles additions
                "dirs" => a.types.add("d")?,
                "files" => a.types.not_dir = true,
                "count" => a.count = true,
                "json" => a.json = true,
                "summary" => a.summary = true,
                "live" => a.live = true,
                "indexed" => a.indexed = true,
                "project" => a.project = true,
                "here" => a.here = true,
                "verbose" | "meta" => a.verbose = true,
                "details" => a.details = true,
                "drive-live" => a.drive_live = true,
                "respect-settings" => a.respect_settings = true,
                _ => return Err(format!("unknown option '--{name}'")),
            }
        }
    }
    if flavor == Flavor::Fd {
        // fd's argument conflicts, checked in the order clap reports them.
        let clash = |cond: bool, a1: &str, a2: &str| -> Result<(), String> {
            if cond { Err(format!("the argument '{a1}' cannot be used with '{a2}'")) } else { Ok(()) }
        };
        let batch = a.exec.as_ref().is_some_and(|x| x.batch);
        let runs = a.exec.is_some() || a.list_details;
        let run_name = if a.list_details { "--list-details" } else if batch { "--exec-batch <cmd>..." } else { "--exec <cmd>..." };
        clash(glob == Some(true) && fixed, "--glob", "--fixed-strings")?;
        clash(exact && glob == Some(true), "--exact", "--glob")?;
        clash(exact && fixed, "--exact", "--fixed-strings")?;
        clash(a.list_details && a.absolute, "--list-details", "--absolute-path")?;
        clash(a.list_details && a.print0, "--print0", "--list-details")?;
        clash(a.list_details && a.format.is_some(), "--format <fmt>", "--list-details")?;
        clash(a.list_details && a.exec.is_some(), if batch { "--exec-batch <cmd>..." } else { "--exec <cmd>..." }, "--list-details")?;
        clash(a.quiet && max_given, "--quiet", "--max-results <count>")?;
        clash(runs && max_given, run_name, "--max-results <count>")?;
        clash(runs && one, run_name, "-1")?;
        clash(runs && a.quiet, run_name, "--quiet")?;
        clash(search_path && !positional.is_empty() && positional.len() > 1, "--search-path <search-path>", "[path]...")?;
        clash(strip_given && (search_path || positional.len() > 1), "--strip-cwd-prefix[=<when>]", "[path]...")?;
        if a.batch_size.is_some() && !batch {
            return Err("the following required arguments were not provided:\n  --exec-batch <cmd>...".into());
        }
    }
    if a.prune && (!a.sizes.is_empty() || exact_depth.is_some()) {
        return Err(format!("the argument '--prune' cannot be used with '{}'", if a.sizes.is_empty() { "--exact-depth" } else { "--size" }));
    }
    a.syntax = a.syntax.or(if glob == Some(true) {
        Some(Syntax::Glob)
    } else if exact {
        Some(Syntax::Exact)
    } else if fixed {
        Some(Syntax::Fixed)
    } else if glob == Some(false) {
        Some(Syntax::Regex)
    } else {
        None
    });
    if let Some(d) = exact_depth {
        if a.max_depth.is_some() || a.min_depth.is_some() {
            return Err("--exact-depth can't be used with --max-depth or --min-depth".into());
        }
        a.min_depth = Some(d);
        a.max_depth = Some(d);
    }
    a.types.finish();
    let mut pos = positional.into_iter();
    a.pattern = pos.next();
    let mut rest: Vec<String> = pos.collect();
    rest.append(&mut a.paths);
    a.paths = rest;
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &[&str]) -> FindArgs {
        parse(Flavor::Fd, &s.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn clusters_values_and_exec() {
        let a = p(&["-HIe", "rs", "main", "src", "-d3", "-x", "echo", "{}", ";", "-t", "f"]);
        assert!(a.hidden && a.no_ignore);
        assert_eq!(a.exts, vec!["rs"]);
        assert_eq!(a.pattern.as_deref(), Some("main"));
        assert_eq!(a.paths, vec!["src"]);
        assert_eq!(a.max_depth, Some(3));
        assert_eq!(p(&["--max-results=4"]).max_results, Some(4));
        // fd's conflicts.
        for bad in [&["-x", "echo", ";", "-1"][..], &["-l", "-0"], &["-l", "--format", "{}"], &["-l", "-q"], &["--prune", "-S", "+1k"]] {
            assert!(parse(Flavor::Fd, &bad.iter().map(|s| s.to_string()).collect::<Vec<_>>()).is_err(), "{bad:?}");
        }
        assert_eq!(a.exec.as_ref().unwrap().cmd, vec!["echo", "{}"]);
        assert!(a.types.file);
        let a = p(&["-1", "--exact-depth", "2", "--", "-x"]);
        assert_eq!(a.max_results, Some(1));
        assert_eq!((a.min_depth, a.max_depth), (Some(2), Some(2)));
        assert_eq!(a.pattern.as_deref(), Some("-x"));
        let a = p(&["-X", "ls", "-l"]);
        assert!(a.exec.unwrap().batch);
        assert!(parse(Flavor::Fd, &["--nope".to_string()]).is_err());
    }
}
