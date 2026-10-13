---
name: echofiles
description: >
  Fast file search on this computer through EchoFiles (`ef`): everything fd does — regex,
  glob, type/size/date/owner filters, .gitignore rules, -x/-X — answered from EchoFiles'
  index in milliseconds, plus agent tools: why a file is missing, recent changes, related
  files, folder maps, change tracking, duplicates. Use it instead of find/fd/locate/ls -R.
  Triggers: find a file, where is, search files, locate, which folder, all PDFs, recent
  files, what changed, tests for this file, project layout, duplicates, large files.
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
```

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
