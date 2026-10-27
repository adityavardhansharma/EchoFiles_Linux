---
name: echofiles
description: >
  Fast file search on this computer through EchoFiles (`ef`): everything fd does — regex,
  glob, type/size/date/owner filters, .gitignore rules, -x/-X — answered from EchoFiles'
  index in milliseconds, plus agent tools: why a file is missing, recent changes, related
  files, folder maps, change tracking, duplicates. Use it instead of find/fd/locate/ls -R.
  Triggers: find a file, where is, search files, locate, which folder, all PDFs, recent
  files, what changed, tests for this file, project layout, duplicates, large files.
  Also pulls photos/videos from the user's paired phone (`ef phone photos today`, `… 5`):
  triggers: photos from my phone, the picture I just took, today's screenshots, last N pics.
---

# EchoFiles — `ef`

`ef` searches the user's files. Indexed folders (usually all of `~`) answer from the
EchoFiles index in a few milliseconds; any other folder is read from disk on the fly. It
accepts every `fd` option, so anything you would do with `fd` or `find`, do with `ef`.

## The commands you'll use most

```sh
ef find report -n 20                 # words, any order, ignoring case/accents; best match first
ef find 'invoice ext:pdf modified:<30d in:~/Documents' -n 20   # search-box filters
ef find '*.rs' --project             # glob, only in the current project (also --here)
ef find 'v[0-9]+' --regex PATH       # regex; -F literal text; -p match the whole path
ef find config -H                    # include dotfiles; -I files hidden by .gitignore; -u both
ef find '*' --project -t f --budget 2000   # fits ~2000 tokens; the rest summarised by folder/type
ef find main.rs --json --details     # JSON with size, date, line count, tags (binary, lockfile…)
ef fd …                              # exactly fd's options, defaults and output format

ef recent --within 1h                # what the user just touched, newest first
ef why path/to/file                  # why a file does or doesn't show up
ef related src/parser.rs             # its tests, header/source pair, styles, docs
ef map                               # project shape: biggest folders, counts, sizes
ef mark  →  ef changed --since ID    # exactly what a build/script added, removed, modified

ef phone photos today                # copy today's photos from the paired phone; prints local paths
ef phone photos 5                    # the newest 5 (also: yesterday, week, 3d, 2026-10-20, A..B)
ef phone photos today --screenshots  # only screenshots (--camera, --chats); --videos adds videos
ef phone photos week --list          # see what's there first, copy nothing
```

## Photos from the phone

`ef phone photos [WHEN…]` copies photos from the user's paired phone and prints one local
path per line, newest first — then open those paths to look at them. Read the full section
in [reference.md](reference.md#ef-phone) before using flags beyond the examples above.

- Be specific: ask for the window or count you need (`today`, `3`, `yesterday --camera`);
  with no WHEN it returns the newest 10. One run copies at most 50 files / 1 GB unless
  you pass `--max` / `--max-size` — raise them only when the user asked for that many.
- Use `--list` first when you don't know how many there are; `--json` gives paths, dates,
  sizes and folders.
- Copies live in EchoFiles' cache; pass `--to DIR` only when the user wants them somewhere.
- If it says the feature is off (exit 3), the user hasn't turned on Settings → AI agents →
  "Let ef fetch phone photos": tell them; don't try to turn it on. If the phone isn't
  nearby, `ef` answers from the saved list and returns only photos copied before — say so.
- Photos are personal: don't upload, share or describe them beyond what the user asked.

## Rules of thumb

- Pass `-n N` (or `--budget TOKENS`) on broad searches; `ef` tells you on stderr how many
  more there were and where they are.
- **Read stderr when nothing matches**: `ef` says what *would* match (with `-H`, `-I`,
  without a filter, as a glob/regex) and suggests close names ("did you mean"). Act on that
  instead of guessing.
- `ef find` prints absolute paths; `ef fd` prints fd-style relative paths.
- Like fd, `.gitignore`d files and dotfiles are skipped unless you add `-I` / `-H` / `-u`.
- A file created in the last seconds may not be indexed yet in rarely watched folders:
  add `--live` to read the disk, or use `ef wait`.
- If `ef` says it's turned off, the user disabled it in EchoFiles → Settings → AI agents:
  fall back to `fd`/`find` and don't try to re-enable it.

For every option, the filter language, all commands, JSON fields, stderr notes and exit
codes, read [reference.md](reference.md). `ef help` and `ef fd --help` print summaries.
