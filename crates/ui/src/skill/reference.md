# `ef` reference

## How it works

- **What's covered.** The folders in EchoFiles → Settings → Search (normally `~`) plus every
  drive mounted on the computer — other disks and partitions, USB drives — unless the user
  turned "Include every drive" off. `ef status` lists them. With no PATH, `ef find`
  searches all of them; any other folder is searched when you name it.
- **Index or disk.** Covered folders are indexed. A search
  inside them reads the index (milliseconds) and checks every result against the disk
  before printing it, so deleted files never appear. Any other folder is walked live.
  `--live` forces a disk walk; `--indexed` forces the index.
- **Freshness.** While EchoFiles runs (it starts at login) it updates the index within
  seconds in Downloads, Desktop, Documents and the folder open in the app, and at least
  every minute elsewhere. When the app isn't running, `ef` reads small folders from disk
  and says on stderr if an answer came from an index that may be stale.
- **Served by the app.** With EchoFiles running, `ef` hands the command to the app, which
  already has everything loaded, and the output appears in your terminal or pipe as usual.
  Commands that run programs (`-x`, `-X`, `-l`), `wait`, `batch` and `index` run in your
  own process.
- **Skipped folders.** The index leaves out the contents of `node_modules`, `.git`, caches
  and similar (Settings → Search). A search that would show their contents (for example
  with `-I` outside a git repository) reads them from disk, so results match fd.

## `ef find PATTERN [PATH…] [OPTIONS]` (also `ef PATTERN …`)

EchoFiles' defaults: results ranked best first (exact name, then prefix, then word start,
then anywhere; shorter names first), absolute paths, and with no PATH every indexed folder
(plus phones, if the user turned phone search on).

**Pattern**

| Form | Meaning |
|---|---|
| `annual report` | all words, any order, ignoring case and accents ("resume" finds "Résumé") |
| `"annual report"` | the exact phrase |
| `*` or nothing | everything (use with filters) |
| `*.rs`, `test_*.py`, `{a,b}*` | a glob — patterns with `*`, `?` or `[` are globs automatically |
| `src/main` | a pattern with `/` matches against the whole path |
| `--regex` / `-r` | regular expression (Rust syntax), smart case |
| `--glob` / `-g` | glob; `**` crosses folders, `{a,b}` alternatives |
| `-F` / `--fixed-strings` | the text literally |
| `--exact` | the whole name (or path with `-p`) equals the text |
| `-p` / `--full-path` | match the pattern against the full path |
| `-s` / `-i` | case-sensitive / insensitive (default: smart case — sensitive only if the pattern has capitals) |
| `--and PATTERN` | another pattern that must also match (repeatable) |

**Filters written inside the pattern** (the EchoFiles search box language; mix freely
with words; quote values with spaces, e.g. `in:"~/My Docs"`):

| Filter | Meaning |
|---|---|
| `ext:pdf`, `ext:jpg,png` | extension(s) |
| `type:file`, `type:folder`, `type:link`, `type:exe`, `type:empty` | entry type |
| `size:>1m`, `size:<500k`, `size:10k..2m`, `size:2kib` | size (k/m/g/t = 1000ⁿ, ki/mi/gi/ti = 1024ⁿ) |
| `modified:<7d`, `modified:>1y`, `modified:today`, `yesterday`, `week`, `month`, `year` | changed within / longer ago than; `modified:2026-01-05` = that day |
| `after:2026-01-05`, `before:2026-02-01` | changed after / before a date (or a duration like `7d`) |
| `in:~/Work` | search only there (repeatable) |
| `depth:2`, `depth:<3`, `depth:>1` | folder depth below the search root |
| `owner:me`, `owner:root` | owner |
| `hidden:yes`, `ignored:yes`, `case:yes` | include dotfiles / .gitignored files / match case |
| `re:REGEX`, `glob:GLOB` | the pattern is a regex / glob |

A `key:value` with any other key is an ordinary word (so `12:30` still searches).

**Option filters** (fd's)

| Option | Meaning |
|---|---|
| `-e EXT`, `--ext EXT` | extension (repeatable) |
| `-t f\|d\|l\|x\|e\|s\|p\|b\|c` | file, directory, symlink, executable, empty, socket, pipe, block/char device (repeatable) |
| `--files` / `--dirs` | anything but folders / folders only |
| `-S +1m`, `-S -500k`, `-S 10KiB` | size at least / at most / exactly |
| `--changed-within 2d`, `--changed-before 2024-01-05` | durations (`10h`, `2weeks`, `1h 30min`) or dates (`2024-01-05`, `2024-01-05 10:00:00`, RFC 3339, `@unix-seconds`) |
| `-o user`, `-o :group`, `-o !root` | owner / group, `!` negates |
| `-d N`, `--min-depth N`, `--exact-depth N` | depth limits |

**What's searched**

| Option | Meaning |
|---|---|
| `-H` | include hidden (dot) files and folders |
| `-I` | include files ignored by `.gitignore`, `.ignore`, `.fdignore`, `.efignore` |
| `-u` | both of the above |
| `-E GLOB` | exclude (gitignore syntax, repeatable) |
| `--no-ignore-vcs`, `--no-require-git`, `--no-ignore-parent`, `--no-global-ignore-file`, `--ignore-file F` | fine-grained ignore control, as in fd |
| `--ignore-contain NAME` | skip folders that contain an entry named NAME |
| `-L` | follow symlinks (loops are detected and reported) |
| `--one-file-system` | stay on the starting drive |
| `--prune` | don't descend into folders that match |
| `--here`, `--project` | the current folder / the current project root (nearest folder with `.git`, `Cargo.toml`, `package.json`, …) |
| `--live`, `--indexed` | read the disk / trust the index |

**Output**

| Option | Meaning |
|---|---|
| `-n N`, `--limit N`, `--max-results N`, `-1` | best N results |
| `--count` | just the number of matches |
| `--json` | one JSON object (below) |
| `--details` | adds `[size · lines · tags · age]` to each line (first 200) |
| `--budget TOKENS` | stop at ~TOKENS of output; then print a summary of where the rest are |
| `--summary` | always print the by-folder / by-type summary |
| `--sort rank\|path\|name\|size\|modified\|none` | order |
| `-v` | say on stderr where the answers came from |
| `-a`, `--relative` | absolute (default) / relative to the current folder |
| `-0`, `--format TEMPLATE`, `-l`, `--color`, `--hyperlink`, `--path-separator` | as in fd |
| `-x CMD ;`, `-X CMD ;`, `--batch-size N`, `-j N` | run a command per result (in parallel) / once for many; placeholders `{}` `{/}` `{//}` `{.}` `{/.}` |
| `-q` | exit 0 if anything matches, 1 if not, print nothing |

## `ef fd [PATTERN] [PATH…] [OPTIONS]`

A drop-in for fd 10.5: same options, same defaults (regex patterns, current folder,
unsorted, relative paths with trailing `/` on folders), same output byte for byte
(colours, `-l`, `--format`, hyperlinks, terminal escaping), same errors and exit codes.
Use it when a script or habit expects fd. `ef find`'s extras don't apply.

## Other commands

| Command | Does | Output |
|---|---|---|
| `ef why PATH [--in ROOT]` | explains whether PATH shows up in searches | the verdicts: hidden (which component), ignored (the rule, file and line), index status (indexed / excluded in Settings / not yet indexed) |
| `ef recent [PATH] [--within 1d] [-n 20] [find options]` | files changed recently, newest first; default PATH is the current project | `path  N min ago  size` |
| `ef related FILE [--in ROOT]` | files that belong with FILE: tests (`test_x`, `x_test`, `x.spec`, `tests/`), header/source, styles, docs, same name elsewhere — closest first | `path  (relation)` |
| `ef map [PATH] [--depth 2] [--lines 60] [-H] [-I] [-u]` | the shape of a folder: biggest subfolders with file counts and sizes; ignored files left out | a tree |
| `ef mark [PATH] [-u]` | remember the folder's state; prints a mark id | `m18f3…` |
| `ef changed --since ID [--json]` | what was added (`+`), removed (`-`) or modified (`~`) since the mark | one change per line |
| `ef wait PATTERN [PATH] [--timeout 2m] [find options]` | block until a match appears (read from disk); exit 1 on timeout | the match(es) |
| `ef batch -c 'find a' -c 'recent'` (or commands on stdin) | several `ef` commands in one call, run in parallel | `### command` then its output, in order |
| `ef dupes [PATH] [--min-size 1k] [-H] [-I]` | files with identical contents, most wasted space first | groups of paths |
| `ef status` | indexed folders, entry counts, index age, whether the app is running | text |
| `ef index [PATH] [--all]` | rebuild the index now (normally the app does this) | stderr summary |
| `ef phone photos [WHEN…]` | copy photos from the paired phone (see [below](#ef-phone)) | local paths, newest first |
| `ef phone status` / `ef phone cache [--clear]` | paired phones and saved photo lists / size of local copies | text |

## `ef phone`

Photos and videos from the user's paired phone (EchoConnect), by date or by count. Needs
Settings → AI agents → **Let ef fetch phone photos** (off by default; exit 3 when off) and
Settings → Phone → **Files**. Copying needs the EchoFiles app running and the phone on the
same Wi-Fi; `ef` talks to the phone through the app.

```sh
ef phone photos today                 # today's photos (local midnight onwards)
ef phone photos yesterday             # yesterday only
ef phone photos today yesterday       # several words widen the window: today and yesterday
ef phone photos week                  # the last 7 calendar days (month: 30)
ef phone photos 3d                    # today and the 2 days before; 2w = 14 days; 6h = rolling 6 hours
ef phone photos 5                     # the newest 5 (same as -n 5); combine: `today 3`
ef phone photos 2026-10-20            # one day;  2026-10-18..2026-10-20  a range (A.. / ..B open)
ef phone photos --since 2026-10-01 --until 2026-10-10   # any ef time (2d, @unix, RFC 3339)
ef phone photos                       # no WHEN: the newest 10
ef phone videos today                 # videos only;  ef phone media today  photos and videos
ef phone photos all --list            # everything, listed, nothing copied
```

| Option | Meaning |
|---|---|
| `-n, --last N` | the newest N of what matches |
| `--camera` `--screenshots` `--chats` `--other` | only those folders (DCIM; anything with "Screenshot"; WhatsApp/Telegram/Signal; the rest). Repeatable |
| `--videos` / `--only-videos` | photos and videos / videos only |
| `--list` (`--dry-run`) | print `date  size  folder  phone/path  → local copy (if any)`; copy nothing |
| `--json` | array of `{phone, phone_id, phone_path, name, taken (UTC RFC 3339), size, video, source, path (local or null), copied_now, error?}` |
| `--to DIR` | also put the copies in DIR (relative to the current folder); never overwrites — `name (2).jpg` |
| `--from auto\|phone\|saved` | `auto` (default): ask the phone for what's new, else the saved list; `phone`: fail if the phone can't be asked; `saved`: no phone round trip at all |
| `--phone NAME` | which phone, by name or id, when several are paired |
| `--max N` / `--max-size 2g` | the most files / bytes one run copies (default 50 / 1 GiB); the newest are kept and stderr says how many more matched |

How it's fast: the app keeps a saved list of the phone's photos (newest first). A run asks
the phone only for pages newer than that list — "the newest 5" is one round trip of a
few milliseconds on Wi-Fi — and photos already copied come straight from
`~/.cache/echofiles/phone/<phone id>/<path on the phone>` without touching the phone.
Four copies run at once.

When the phone is away (or EchoFiles isn't running), the saved list and the phone file
index answer; stderr says so, and only photos already on the laptop are returned (the rest
are reported as `not on the laptop yet`).

Output: one absolute local path per line, newest first (with `--to`, the paths in DIR).
Problems go to stderr as `ef: PHONE/PATH: reason`. Exit `0` everything asked for was
returned, `1` nothing matched or some files couldn't be copied, `2` bad arguments, `3`
turned off in Settings.

## `--json`

```json
{
  "total": 54,
  "sources": [{"root": "/home/me", "from": "index", "watched": true,
               "skipped_folders_read_live": 0, "saved_seconds_ago": 12}],
  "results": [{"path": "/home/me/Docs/report.pdf", "type": "file", "size": 48213,
               "modified": "2026-10-12T09:14:03Z", "tags": ["binary"]},
              {"path": "/home/me/Work/notes.md", "type": "file", "size": 912,
               "modified": "2026-10-13T08:01:44Z", "lines": 31}],
  "shown": 20,
  "by_folder": [{"/home/me/Docs": 31}],
  "by_type": [{".pdf": 40}],
  "errors": []
}
```

- `from` is `index` or `live`; `watched` means the EchoFiles app keeps that index current.
- `type`: `file`, `dir`, `symlink`, `socket`, `pipe`, `block-device`, `char-device`, `other`.
- `size` is absent for folders. `lines` and `tags` appear with `--details` (first 200
  results); `lines` is absent for binary or very large files.
- `tags`: `binary`, `lockfile`, `minified`, `generated`, `build-output`, `large` (>1 MB) —
  usually files you shouldn't read whole.
- `by_folder` / `by_type` appear when not every match was shown.

## Notes on stderr (`ef find`)

- `showing the best N of M (--limit to change)` — more matched than were shown.
- `no matches in ~ (index)` followed by `N would match if you include hidden files (-H)`,
  `… if you drop the extension filter`, `… read the pattern as a glob (-g)`, … and
  `did you mean: "search.rs", …` — act on these.
- `answered from the index of ~ (saved 3 h ago; EchoFiles isn't running…)` — consider `--live`.
- `N indexed results were deleted since the last index update and were left out`.

## Exit codes

`0` success (also when nothing matched, except with `-q`), `1` an error or nothing matched
with `-q`, `2` invalid options, `3` ef is turned off in Settings → AI agents.
