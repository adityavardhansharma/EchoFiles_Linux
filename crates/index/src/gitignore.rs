//! `.gitignore`-format rules: parsing, matching, and the stack of rule files that applies
//! to one folder.
//!
//! Precedence (the same as git, ripgrep and fd):
//! - Inside one file, the last matching line wins; `!` re-includes.
//! - A deeper folder's file beats a shallower one of the same kind.
//! - Kinds, strongest first: `--exclude` patterns, `.fdignore`/`.efignore`, `.ignore`,
//!   `.gitignore`, `.git/info/exclude`, the global git excludes file, `--ignore-file`s.
//! - Git rules only count inside a git repository (unless `require_git` is off), and only
//!   up to that repository's root.
//! - Once a folder is ignored nothing below it is visited, so nothing inside can be
//!   re-included (the walkers prune; [`Stack::check`] only looks at one entry).

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::glob::{Glob, GlobOptions};

/// One line of a rule file.
#[derive(Clone, Debug)]
pub struct Rule {
    glob: Glob,
    /// `!pattern`: re-include.
    pub negated: bool,
    /// `pattern/`: folders only.
    dir_only: bool,
    /// Has a `/` before the end: matched against the path relative to the file's folder;
    /// otherwise against the name alone, at any depth.
    anchored: bool,
    /// 1-based line number in its file.
    pub line: u32,
}

impl Rule {
    pub fn pattern(&self) -> &str {
        &self.glob.source
    }
}

/// The rules from one file (or one `--exclude` list), relative to one folder.
#[derive(Debug)]
pub struct RuleSet {
    /// Absolute folder the patterns are relative to, without a trailing `/`.
    dir: Vec<u8>,
    /// Where the rules came from (for `ef why`).
    pub source: PathBuf,
    rules: Vec<Rule>,
    /// Rules that are a plain name (`node_modules`, `*.log` is not): name → rule indexes.
    by_name: std::collections::HashMap<Vec<u8>, Vec<u32>>,
    /// Indexes of every other rule, ascending.
    other: Vec<u32>,
    case_insensitive: bool,
}

/// What one rule set (or the whole stack) says about a path.
#[derive(Clone, Copy, Debug)]
pub enum Verdict<'a> {
    /// No rule matched.
    None,
    Ignore(&'a RuleSet, &'a Rule),
    /// Re-included by a `!` rule.
    Whitelist(&'a RuleSet, &'a Rule),
}

impl Verdict<'_> {
    pub fn is_none(&self) -> bool {
        matches!(self, Verdict::None)
    }

    pub fn is_ignore(&self) -> bool {
        matches!(self, Verdict::Ignore(..))
    }

    fn or(self, other: impl FnOnce() -> Self) -> Self {
        if self.is_none() { other() } else { self }
    }
}

fn trim_trailing_spaces(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    while end > 0 && line[end - 1] == b' ' {
        // `\ ` keeps the space.
        if end >= 2 && line[end - 2] == b'\\' {
            break;
        }
        end -= 1;
    }
    &line[..end]
}

impl RuleSet {
    /// Parse rule text whose patterns are relative to `dir`.
    pub fn parse(dir: &[u8], source: PathBuf, text: &[u8], case_insensitive: bool) -> RuleSet {
        let dir = if dir.len() > 1 { dir.strip_suffix(b"/").unwrap_or(dir) } else { dir };
        let mut set =
            RuleSet { dir: dir.to_vec(), source, rules: Vec::new(), by_name: Default::default(), other: Vec::new(), case_insensitive };
        for (n, raw) in text.split(|&b| b == b'\n').enumerate() {
            let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
            set.add(raw, n as u32 + 1);
        }
        set
    }

    /// Rules given one per string (fd's `--exclude`), relative to `dir`.
    pub fn from_patterns(dir: &[u8], source: PathBuf, patterns: &[String], case_insensitive: bool) -> RuleSet {
        let mut set = RuleSet::parse(dir, source, b"", case_insensitive);
        for (n, p) in patterns.iter().enumerate() {
            set.add(p.as_bytes(), n as u32 + 1);
        }
        set
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    fn add(&mut self, raw: &[u8], line: u32) {
        if raw.is_empty() || raw[0] == b'#' {
            return;
        }
        let mut p = trim_trailing_spaces(raw);
        let mut negated = false;
        if let Some(rest) = p.strip_prefix(b"!") {
            negated = true;
            p = rest;
        } else if p.starts_with(b"\\!") || p.starts_with(b"\\#") {
            p = &p[1..];
        }
        let mut dir_only = false;
        if p.len() > 1 && p.ends_with(b"/") && !p.ends_with(b"\\/") {
            dir_only = true;
            p = &p[..p.len() - 1];
        }
        if p.is_empty() || p == b"/" {
            return;
        }
        let anchored = p.contains(&b'/');
        let p = p.strip_prefix(b"/").unwrap_or(p);
        let opts = GlobOptions { case_insensitive: self.case_insensitive, braces: false, nonempty_trailing_recursive: true };
        let Ok(glob) = Glob::from_bytes(p, opts) else { return };
        let idx = self.rules.len() as u32;
        let plain = !anchored && !p.iter().any(|b| matches!(b, b'*' | b'?' | b'[' | b'\\'));
        if plain && !self.case_insensitive {
            self.by_name.entry(p.to_vec()).or_default().push(idx);
        } else {
            self.other.push(idx);
        }
        self.rules.push(Rule { glob, negated, dir_only, anchored, line });
    }

    /// The last rule matching `abs` (an absolute path below or at [`dir`]), if any.
    pub fn matched(&self, abs: &[u8], is_dir: bool) -> Verdict<'_> {
        if self.rules.is_empty() {
            return Verdict::None;
        }
        let rel = match abs.strip_prefix(self.dir.as_slice()) {
            Some(r) if self.dir == b"/" => r,
            Some(r) if r.first() == Some(&b'/') => &r[1..],
            Some([]) => return Verdict::None,
            _ => abs,
        };
        let name = rel.rsplit(|&b| b == b'/').next().unwrap_or(rel);
        let ok = |r: &Rule| !r.dir_only || is_dir;
        let mut best: Option<u32> = None;
        if let Some(ids) = self.by_name.get(name) {
            best = ids.iter().rev().copied().find(|&i| ok(&self.rules[i as usize]));
        }
        for &i in self.other.iter().rev() {
            if best.is_some_and(|b| b > i) {
                break;
            }
            let r = &self.rules[i as usize];
            if ok(r) && r.glob.matches(if r.anchored { rel } else { name }) {
                best = Some(i);
                break;
            }
        }
        match best {
            None => Verdict::None,
            Some(i) => {
                let r = &self.rules[i as usize];
                if r.negated { Verdict::Whitelist(self, r) } else { Verdict::Ignore(self, r) }
            }
        }
    }
}

/// Which rule files are read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IgnoreOptions {
    /// `.gitignore`, `.git/info/exclude` and the global git excludes file.
    pub vcs: bool,
    /// `.ignore`, `.fdignore` and `.efignore`.
    pub dot: bool,
    /// Read rule files in the folders above the search root too.
    pub parents: bool,
    /// Git rules only apply inside a git repository.
    pub require_git: bool,
    /// fd's and EchoFiles' global ignore files (`~/.config/fd/ignore`,
    /// `~/.config/echofiles/ignore`).
    pub global: bool,
    pub case_insensitive: bool,
}

impl Default for IgnoreOptions {
    fn default() -> Self {
        IgnoreOptions { vcs: true, dot: true, parents: true, require_git: true, global: true, case_insensitive: false }
    }
}

impl IgnoreOptions {
    pub fn none() -> IgnoreOptions {
        IgnoreOptions { vcs: false, dot: false, parents: false, require_git: true, global: false, case_insensitive: false }
    }

    pub fn any(&self) -> bool {
        self.vcs || self.dot
    }
}

/// Names of the per-folder rule files, strongest first.
pub const CUSTOM_FILES: [&[u8]; 2] = [b".efignore", b".fdignore"];
pub const DOT_FILE: &[u8] = b".ignore";
pub const GIT_FILE: &[u8] = b".gitignore";

/// The rule files of one folder.
#[derive(Debug, Default)]
pub struct DirRules {
    custom: Vec<RuleSet>,
    dot: Option<RuleSet>,
    git: Option<RuleSet>,
    exclude: Option<RuleSet>,
    /// The folder holds `.git` (a repository root, or a worktree/submodule with a `.git` file).
    pub has_git: bool,
}

impl DirRules {
    /// Read `dir`'s rule files. `has` says whether a name exists in the folder (from a
    /// listing already in hand), so absent files cost no syscall.
    pub fn load(dir: &[u8], has: &dyn Fn(&[u8]) -> bool, opts: &IgnoreOptions) -> Option<DirRules> {
        let mut r = DirRules { has_git: has(b".git"), ..Default::default() };
        let read = |name: &[u8]| -> Option<RuleSet> {
            if !has(name) {
                return None;
            }
            let mut p = dir.to_vec();
            if p.last() != Some(&b'/') {
                p.push(b'/');
            }
            p.extend_from_slice(name);
            let path = PathBuf::from(OsStr::from_bytes(&p));
            let text = std::fs::read(&path).ok()?;
            let set = RuleSet::parse(dir, path, &text, opts.case_insensitive);
            (!set.is_empty()).then_some(set)
        };
        if opts.dot {
            r.custom = CUSTOM_FILES.iter().filter_map(|n| read(n)).collect();
            r.dot = read(DOT_FILE);
        }
        if opts.vcs {
            r.git = read(GIT_FILE);
            if r.has_git {
                let mut p = dir.to_vec();
                p.extend_from_slice(b"/.git/info/exclude");
                let path = PathBuf::from(OsStr::from_bytes(&p));
                if let Ok(text) = std::fs::read(&path) {
                    let set = RuleSet::parse(dir, path, &text, opts.case_insensitive);
                    r.exclude = (!set.is_empty()).then_some(set);
                }
            }
        }
        let empty = r.custom.is_empty() && r.dot.is_none() && r.git.is_none() && r.exclude.is_none() && !r.has_git;
        (!empty).then_some(r)
    }

    /// Read a folder's rule files by checking the disk for each name.
    pub fn load_from_disk(dir: &Path, opts: &IgnoreOptions) -> Option<DirRules> {
        let has = |name: &[u8]| std::fs::symlink_metadata(dir.join(OsStr::from_bytes(name))).is_ok();
        DirRules::load(dir.as_os_str().as_bytes(), &has, opts)
    }
}

/// Rules that hold for the whole search, not one folder.
#[derive(Debug, Default)]
pub struct Globals {
    /// `--exclude` patterns, relative to the search root. Checked before everything else.
    pub overrides: Option<RuleSet>,
    /// The global git excludes file (`core.excludesFile`, else `~/.config/git/ignore`).
    pub git_global: Option<RuleSet>,
    /// `--ignore-file`s and the global fd/EchoFiles ignore files, weakest of all.
    pub explicit: Vec<RuleSet>,
    pub opts: IgnoreOptions,
}

impl Globals {
    pub fn new(opts: IgnoreOptions, root: &Path, excludes: &[String], ignore_files: &[PathBuf]) -> Globals {
        let ci = opts.case_insensitive;
        let overrides =
            (!excludes.is_empty()).then(|| RuleSet::from_patterns(root.as_os_str().as_bytes(), PathBuf::from("--exclude"), excludes, ci));
        let mut explicit = Vec::new();
        let rules_file = |path: &Path| -> Option<RuleSet> {
            let text = std::fs::read(path).ok()?;
            let dir = path.parent().unwrap_or(Path::new("/"));
            let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
            Some(RuleSet::parse(dir.as_os_str().as_bytes(), path.to_path_buf(), &text, ci))
        };
        if opts.global && opts.dot {
            let cfg = xdg_config();
            for p in [cfg.join("fd/ignore"), cfg.join("echofiles/ignore")] {
                explicit.extend(rules_file(&p));
            }
        }
        for p in ignore_files {
            explicit.extend(rules_file(p));
        }
        let git_global = if opts.vcs && opts.global { global_git_excludes().and_then(|p| rules_file(&p)) } else { None };
        Globals { overrides, git_global, explicit, opts }
    }
}

fn xdg_config() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| crate::config::home().join(".config"))
}

/// `core.excludesFile` from `~/.gitconfig` or `$XDG_CONFIG_HOME/git/config`, else the
/// default `$XDG_CONFIG_HOME/git/ignore`.
fn global_git_excludes() -> Option<PathBuf> {
    let home = crate::config::home();
    for cfg in [home.join(".gitconfig"), xdg_config().join("git/config")] {
        let Ok(text) = std::fs::read_to_string(&cfg) else { continue };
        let mut in_core = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_core = line.trim_start_matches('[').trim_end_matches(']').trim().eq_ignore_ascii_case("core");
                continue;
            }
            if !in_core {
                continue;
            }
            if let Some((k, v)) = line.split_once('=')
                && k.trim().eq_ignore_ascii_case("excludesfile")
            {
                let v = v.trim().trim_matches('"');
                return Some(crate::config::expand(v));
            }
        }
    }
    let p = xdg_config().join("git/ignore");
    p.exists().then_some(p)
}

/// The rule files in effect in one folder: a linked list from the folder up to the search
/// root (and, with `parents`, above it). Folders without rule files share their parent's
/// stack, so a walk allocates one node per rule-file folder, not per folder.
#[derive(Debug)]
pub struct Stack {
    rules: Option<Arc<DirRules>>,
    parent: Option<Arc<Stack>>,
    /// Some folder on the way up holds `.git`.
    any_git: bool,
    /// The node belongs to a folder above the search root.
    above_root: bool,
    globals: Arc<Globals>,
}

impl Stack {
    /// The stack for the search root: the root's own rule files and, with
    /// `opts.parents`, those of every folder above it.
    pub fn root(root: &Path, globals: Arc<Globals>, root_rules: Option<DirRules>) -> Arc<Stack> {
        let mut chain: Vec<DirRules> = Vec::new();
        if globals.opts.parents && globals.opts.any() {
            let mut p = root.parent();
            while let Some(dir) = p {
                if let Some(r) = DirRules::load_from_disk(dir, &globals.opts) {
                    chain.push(r);
                }
                p = dir.parent();
            }
        } else if globals.opts.vcs {
            // Without parent rule files we still need to know whether we're in a repository.
            let mut p = root.parent();
            while let Some(dir) = p {
                if dir.join(".git").exists() {
                    chain.push(DirRules { has_git: true, ..Default::default() });
                    break;
                }
                p = dir.parent();
            }
        }
        let mut stack = Arc::new(Stack { rules: None, parent: None, any_git: false, above_root: true, globals: globals.clone() });
        for r in chain.into_iter().rev() {
            let any_git = stack.any_git || r.has_git;
            stack = Arc::new(Stack { rules: Some(Arc::new(r)), parent: Some(stack), any_git, above_root: true, globals: globals.clone() });
        }
        match root_rules {
            Some(r) => stack.push(r),
            None => stack,
        }
    }

    /// The stack for a child folder with its own rule files.
    pub fn push(self: &Arc<Stack>, rules: DirRules) -> Arc<Stack> {
        Arc::new(Stack {
            any_git: self.any_git || rules.has_git,
            rules: Some(Arc::new(rules)),
            parent: Some(self.clone()),
            above_root: false,
            globals: self.globals.clone(),
        })
    }

    pub fn globals(&self) -> &Globals {
        &self.globals
    }

    fn nodes(&self) -> impl Iterator<Item = &Stack> {
        std::iter::successors(Some(self), |s| s.parent.as_deref())
    }

    /// What the rule files say about `abs` (an entry inside this stack's folder).
    pub fn check(&self, abs: &[u8], is_dir: bool) -> Verdict<'_> {
        let g = &*self.globals;
        if let Some(o) = &g.overrides {
            let v = o.matched(abs, is_dir);
            if !v.is_none() {
                return v;
            }
        }
        if !g.opts.any() && g.explicit.is_empty() {
            return Verdict::None;
        }
        let any_git = !g.opts.require_git || self.any_git;
        let (mut custom, mut dot, mut git, mut exclude) = (Verdict::None, Verdict::None, Verdict::None, Verdict::None);
        let mut saw_git = false;
        for node in self.nodes() {
            let Some(r) = node.rules.as_deref() else { continue };
            if node.above_root && !g.opts.parents && !r.has_git {
                continue;
            }
            if custom.is_none() {
                for c in &r.custom {
                    custom = c.matched(abs, is_dir);
                    if !custom.is_none() {
                        break;
                    }
                }
            }
            if dot.is_none()
                && let Some(d) = &r.dot
            {
                dot = d.matched(abs, is_dir);
            }
            if any_git && !saw_git {
                if git.is_none()
                    && let Some(s) = &r.git
                {
                    git = s.matched(abs, is_dir);
                }
                if exclude.is_none()
                    && let Some(s) = &r.exclude
                {
                    exclude = s.matched(abs, is_dir);
                }
            }
            saw_git |= r.has_git;
        }
        custom
            .or(|| dot)
            .or(|| git)
            .or(|| exclude)
            .or(|| match (&g.git_global, any_git) {
                (Some(s), true) => s.matched(abs, is_dir),
                _ => Verdict::None,
            })
            .or(|| {
                for e in g.explicit.iter().rev() {
                    let v = e.matched(abs, is_dir);
                    if !v.is_none() {
                        return v;
                    }
                }
                Verdict::None
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(text: &str) -> RuleSet {
        RuleSet::parse(b"/r", PathBuf::from("/r/.gitignore"), text.as_bytes(), false)
    }

    fn ig(s: &RuleSet, p: &str, dir: bool) -> Option<bool> {
        match s.matched(format!("/r/{p}").as_bytes(), dir) {
            Verdict::None => None,
            Verdict::Ignore(..) => Some(true),
            Verdict::Whitelist(..) => Some(false),
        }
    }

    #[test]
    fn rules() {
        let s = set("# c\n*.log\n!keep.log\ntarget/\n/top\ndoc/*.txt\n\\#hash\n\\!bang\ntrail \\ \n**/deep/x\nabc/**\n");
        assert_eq!(ig(&s, "a.log", false), Some(true));
        assert_eq!(ig(&s, "sub/a.log", false), Some(true));
        assert_eq!(ig(&s, "keep.log", false), Some(false));
        assert_eq!(ig(&s, "target", true), Some(true));
        assert_eq!(ig(&s, "target", false), None);
        assert_eq!(ig(&s, "x/target", true), Some(true));
        assert_eq!(ig(&s, "top", false), Some(true));
        assert_eq!(ig(&s, "x/top", false), None);
        assert_eq!(ig(&s, "doc/a.txt", false), Some(true));
        assert_eq!(ig(&s, "doc/x/a.txt", false), None);
        assert_eq!(ig(&s, "x/doc/a.txt", false), None);
        assert_eq!(ig(&s, "#hash", false), Some(true));
        assert_eq!(ig(&s, "!bang", false), Some(true));
        assert_eq!(ig(&s, "trail  ", false), Some(true));
        assert_eq!(ig(&s, "deep/x", false), Some(true));
        assert_eq!(ig(&s, "a/b/deep/x", false), Some(true));
        assert_eq!(ig(&s, "abc", true), None);
        assert_eq!(ig(&s, "abc/d", false), Some(true));
    }

    #[test]
    fn last_rule_wins() {
        let s = set("foo\n!foo\n");
        assert_eq!(ig(&s, "foo", false), Some(false));
        let s = set("!foo\nfoo\n");
        assert_eq!(ig(&s, "foo", false), Some(true));
        let s = set("f*\n!foo\nfoo\n");
        assert_eq!(ig(&s, "foo", false), Some(true));
        let s = set("foo\n!f*\n");
        assert_eq!(ig(&s, "foo", false), Some(false));
    }
}
