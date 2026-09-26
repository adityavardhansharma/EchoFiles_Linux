# EchoFiles vs Nautilus — 2026-09-26

Machine: Ryzen 9 4900HS, Vega iGPU + GTX 1660Ti, btrfs (zstd) on NVMe, Omarchy / Hyprland,
warm page cache. EchoFiles M1 build (release, x86-64-v3). Nautilus 50.2.2 (system default).

Method (`python3 bench/compare.py 3`): fresh process per run, one warm-up run discarded,
median of 3. "Window" = until Hyprland maps the window. "Content stable" = until three
consecutive `grim` captures of the window are identical — listing finished and spinners
gone. Both include Hyprland's open animation (~0.45 s of changing pixels), so the gap between
the apps is what matters, not the absolute value. RSS and CPU time are read from `/proc` at
that moment, summed over the process tree.

| Folder | App | Window | Content stable | RSS | CPU time |
|---|---|---|---|---|---|
| ~/Projects (small) | EchoFiles | 93 ms | 564 ms | 141 MB | 110 ms |
| | Nautilus | 512 ms | 1627 ms | 287 MB | 570 ms |
| 10k files | EchoFiles | 87 ms | 575 ms | 159 MB | 140 ms |
| | Nautilus | 505 ms | 1481 ms | 315 MB | 2130 ms |
| 100k files | EchoFiles | 117 ms | 638 ms | 221 MB | 430 ms |
| | Nautilus | 504 ms | 10787 ms | 416 MB | 15480 ms |

EchoFiles internal numbers for the same build (`ECHOFILES_TIMING=1`): first frame 67–75 ms
after process start (91 ms with the 100k folder); 100k listing names 24–30 ms, sort 16–19 ms,
details 13–16 ms; 600-frame scroll through 100k rows p50 16.67 ms, p99 17.5 ms, max 17.8 ms
(locked to the 60 Hz display).

Not measured here: scroll smoothness in Nautilus (no in-app frame counter), cold cache after
reboot, NTFS partitions (M2).
