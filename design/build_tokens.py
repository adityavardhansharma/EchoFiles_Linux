#!/usr/bin/env python3
"""EchoFiles tokens, derived from Omarchy themes.

This is the reference implementation of the Omarchy -> EchoFiles colour derivation.
The Rust app runs the same rules at startup (and on Omarchy's theme-set hook) against
~/.local/state/omarchy/current/theme/colors.toml, so every Omarchy theme -- including
ones installed later -- gets a complete, contrast-checked EchoFiles palette.

Writes design/system/project/tokens.json (design-system format) and design/tokens.json.
"""
import json, pathlib, tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
OMARCHY = pathlib.Path("/usr/share/omarchy/themes")

# Brand theme ("Echo"): the logo's navy tile with its three sheet colours.
ECHO = {
    "mode": "dark", "accent": "#4cc3ff", "selection": "#1b2656", "muted": "#2a3566",
    "background": "#0b1030", "dark_background": "#080c26", "darker_background": "#050819",
    "lighter_background": "#131a45", "foreground": "#c9d3f5", "dark_foreground": "#5d6799",
    "bright_foreground": "#eef2ff", "red": "#ff6b81", "yellow": "#ffc857", "orange": "#ff8a3d",
    "green": "#5ee6a0", "cyan": "#3dd9f5", "blue": "#4c8dff", "magenta": "#a56bff",
}
THEMES = [("echo", "Echo", None), ("tokyo-night", "Tokyo Night", "tokyo-night"),
          ("catppuccin-latte", "Catppuccin Latte", "catppuccin-latte"), ("gruvbox", "Gruvbox", "gruvbox"),
          ("rose-pine", "Rosé Pine", "rose-pine"), ("everforest", "Everforest", "everforest")]

# ---------------------------------------------------------------- colour maths
def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))

def hexc(c):
    return "#" + "".join(f"{round(max(0, min(1, v)) * 255):02x}" for v in c)

def mix(a, b, t):
    """t=0 -> a, t=1 -> b (sRGB)."""
    A, B = rgb(a), rgb(b)
    return hexc(tuple(x + (y - x) * t for x, y in zip(A, B)))

def lum(h):
    def ch(v):
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (ch(v) for v in rgb(h))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b

def contrast(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)

def ensure(color, grounds, target, toward):
    """Move `color` toward `toward` until it reaches `target` contrast on every ground."""
    for i in range(0, 101):
        c = mix(color, toward, i / 100)
        if all(contrast(c, g) >= target for g in grounds):
            return c
    return toward

def rgba(h, a):
    r, g, b = (round(v * 255) for v in rgb(h))
    return f"rgba({r}, {g}, {b}, {a})"

def best_on(fill, candidates):
    return max(candidates, key=lambda c: contrast(c, fill))

# ---------------------------------------------------------------- derivation
def derive(p):
    dark = p.get("mode", "dark") == "dark"
    bg = p["background"]
    sunken = p.get("dark_background", mix(bg, "#000000", 0.2))
    deep = p.get("darker_background", mix(bg, "#000000", 0.35))
    raised = p.get("lighter_background", mix(bg, "#ffffff", 0.06))
    if not dark:  # light themes: raised surfaces are the brightest, not the "lighter" key
        raised = mix(bg, "#ffffff", 0.6)
    fg = p["foreground"]
    extreme = "#ffffff" if dark else "#000000"
    surfaces = [bg, sunken, deep, raised]

    selection = p.get("selection", mix(p["accent"], bg, 0.75))
    ink = ensure(fg, surfaces + [selection], 7.0, extreme)
    selection = ensure(selection, [ink], 7.0, bg) if contrast(ink, selection) < 7 else selection
    strong = ensure(p.get("bright_foreground", fg), surfaces + [selection], 7.0, extreme)
    muted = ensure(mix(fg, bg, 0.45), surfaces + [selection], 4.5, ink)
    faint = p.get("dark_foreground", mix(fg, bg, 0.6))

    accent = p["accent"]
    accent_ink = ensure(accent, surfaces + [selection], 4.5, extreme)
    focus = ensure(accent, surfaces + [selection], 3.0, extreme)
    on_accent = best_on(accent, [deep, "#ffffff", "#000000"])
    accent_soft = mix(bg, accent, 0.16)

    out = {
        "bg": bg, "bg-sunken": sunken, "bg-deep": deep, "bg-raised": raised,
        "line": mix(bg, fg, 0.14 if dark else 0.16),
        "line-strong": ensure(mix(bg, fg, 0.3), [bg, sunken, raised], 3.0, ink),
        "ink": ink, "ink-strong": strong, "ink-muted": muted, "ink-faint": faint,
        "accent": accent, "accent-ink": accent_ink, "on-accent": on_accent,
        "accent-soft": accent_soft, "selection": selection, "focus-ring": focus,
        "state-hover": rgba(fg, 0.08), "state-press": rgba(fg, 0.22), "state-active": rgba(fg, 0.18),
        "scrim": rgba(deep, 0.55),
    }
    for name, key in (("success", "green"), ("warning", "yellow"), ("danger", "red"), ("info", "cyan")):
        c = p[key]
        soft = mix(bg, c, 0.14)
        out[name] = c
        out[f"{name}-soft"] = soft
        out[f"{name}-ink"] = ensure(c, [bg, raised, soft], 4.5, extreme)
        out[f"on-{name}"] = best_on(c, [deep, "#ffffff", "#000000"])
    for name, key in (("world-linux", "orange"), ("world-windows", "blue"), ("world-phone", "green")):
        out[name] = ensure(p[key], [sunken, bg, raised], 3.0, extreme)
    # icon palette slots (see design/build_icons.py)
    y = p["yellow"]
    out.update({
        "icon-folder": y, "icon-folder-back": mix(y, deep, 0.16),
        "icon-folder-glyph": mix(y, "#000000", 0.62),
        "icon-blue": p["blue"], "icon-blue-deep": mix(p["blue"], "#000000", 0.25),
        "icon-green": p["green"], "icon-red": p["red"], "icon-orange": p.get("orange", p["yellow"]),
        "icon-purple": p["magenta"], "icon-yellow": y,
        "icon-slate": mix(fg, bg, 0.35) if dark else mix(fg, bg, 0.15),
        "icon-slate-deep": mix(fg, bg, 0.62) if dark else fg,
        "icon-paper": mix(fg, bg, 0.12) if dark else mix(bg, fg, 0.1),
        "icon-paper-fold": mix(fg, bg, 0.45) if dark else mix(bg, fg, 0.3),
        "icon-on": "#ffffff" if not dark else mix(fg, "#ffffff", 0.7),
    })
    out["shadow-float"] = f"0 12px 32px {rgba('#000000', 0.45 if dark else 0.16)}, 0 2px 6px {rgba('#000000', 0.3 if dark else 0.08)}"
    out["shadow-pop"] = f"0 6px 18px {rgba('#000000', 0.38 if dark else 0.12)}"
    return out

def checks(name, t):
    pairs = [("ink", g) for g in ("bg", "bg-sunken", "bg-deep", "bg-raised", "selection")]
    pairs += [("ink-muted", g) for g in ("bg", "bg-sunken", "bg-raised", "selection")]
    pairs += [("accent-ink", g) for g in ("bg", "bg-raised", "selection")] + [("on-accent", "accent")]
    pairs += [(f"{s}-ink", g) for s in ("success", "warning", "danger", "info") for g in ("bg", f"{s}-soft")]
    pairs += [(f"on-{s}", s) for s in ("success", "warning", "danger", "info")]
    need = {"ink": 7.0}
    fails = []
    for a, b in pairs:
        target = 4.5 if not a.startswith("ink") or a == "ink-muted" else 7.0
        c = contrast(t[a], t[b])
        if c < target:
            fails.append(f"{a} on {b} {c:.2f}")
    for a, b in [("focus-ring", g) for g in ("bg", "bg-raised", "selection")] + [("line-strong", "bg"), ("world-linux", "bg-sunken"), ("world-windows", "bg-sunken"), ("world-phone", "bg-sunken")]:
        c = contrast(t[a], t[b])
        if c < 3.0:
            fails.append(f"{a} on {b} {c:.2f}")
    print(f"{name:18} {'OK' if not fails else 'FAIL: ' + '; '.join(fails)}")
    return fails

USAGE = {
    "bg": "Main file area and window ground. Omarchy `background`.",
    "bg-sunken": "Sidebar, preview pane and inactive dual-pane side. Omarchy `dark_background`.",
    "bg-deep": "Tab strip, status bar and text-field wells. Omarchy `darker_background`.",
    "bg-raised": "Menus, popovers, dialogs, toasts, command palette. Omarchy `lighter_background` (light themes: background lifted toward white).",
    "line": "Hairlines: row separators, pane dividers, section rules. Decorative, never the only boundary of a control.",
    "line-strong": "Control borders (fields, buttons, checkboxes) and resize handles. At least 3:1 on bg, bg-sunken and bg-raised.",
    "ink": "Filenames and all primary text, on every surface and on selection. At least 7:1.",
    "ink-strong": "Headings, the focused filename, dialog titles. Omarchy `bright_foreground`.",
    "ink-muted": "Metadata columns (size, date, kind), captions, sidebar section labels, placeholder text. At least 4.5:1 on every surface and on selection.",
    "ink-faint": "Disabled text and decorative marks only. Omarchy `dark_foreground`; not guaranteed legible.",
    "accent": "The one active thing: primary button fill, active tab underline, drag target outline, progress fill. Omarchy `accent`.",
    "accent-ink": "Accent used as text or glyph (links, active sidebar item label, sort-direction arrow), on every surface and selection.",
    "on-accent": "Text and glyphs on an accent fill.",
    "accent-soft": "Tinted ground behind accent text: drop-target folder, current-location chip.",
    "selection": "Selected rows and tiles. Omarchy `selection`; ink stays 7:1 on it.",
    "focus-ring": "Keyboard cursor and focus outline (1px inset on rows, 2px outline on controls). At least 3:1 on every surface and selection.",
    "state-hover": "Hover layer: foreground at 8% (Omarchy controls hover-cursor-fill-alpha).",
    "state-active": "Active/checked layer: foreground at 18% (Omarchy selected-fill-alpha).",
    "state-press": "Pressed layer: foreground at 22% (Omarchy pressed-fill-alpha).",
    "scrim": "Behind dialogs and the command palette. Omarchy scrim-alpha 0.5 over darker_background.",
    "success": "Completed transfer, mounted state, verified checksum. Omarchy `green`.",
    "success-soft": "Ground of success banners and pills.",
    "success-ink": "Success as text or glyph on bg, bg-raised or success-soft.",
    "on-success": "Text on a success fill.",
    "warning": "Read-only / dirty-volume / Fast Startup state. Omarchy `yellow`.",
    "warning-soft": "Ground of warning banners and pills.",
    "warning-ink": "Warning as text or glyph on bg, bg-raised or warning-soft.",
    "on-warning": "Text on a warning fill.",
    "danger": "Permanent delete, failed operation, broken link. Omarchy `red`.",
    "danger-soft": "Ground of error banners and pills.",
    "danger-ink": "Danger as text or glyph on bg, bg-raised or danger-soft.",
    "on-danger": "Text on a danger fill (Delete permanently button).",
    "info": "Neutral notices: cloud-only placeholder, +ADS stream, hint banners. Omarchy `cyan`.",
    "info-soft": "Ground of info banners and pills.",
    "info-ink": "Info as text or glyph on bg, bg-raised or info-soft.",
    "on-info": "Text on an info fill.",
    "world-linux": "Linux section mark and Linux drive usage bars. Omarchy `orange`; 3:1 on the sidebar.",
    "world-windows": "Windows section mark and NTFS drive usage bars. Omarchy `blue`; 3:1 on the sidebar.",
    "world-phone": "Phone section mark (v0.3+). Omarchy `green`; 3:1 on the sidebar.",
    "brand-ember": "Logo back sheet. Brand surfaces only (About, onboarding, app icon) — never UI state.",
    "brand-violet": "Logo middle sheet. Brand surfaces only.",
    "brand-sky": "Logo front sheet. Brand surfaces only.",
    "brand-navy": "Logo tile ground. Brand surfaces only.",
    "icon-folder": "Folder front face in colour icons. Omarchy `yellow`.",
    "icon-folder-back": "Folder back sheet and tab.",
    "icon-folder-glyph": "Glyph printed on a folder (download arrow, lock, gear…).",
    "icon-blue": "Colour-icon slot {blue}: documents, cloud, phone, home. Omarchy `blue`.",
    "icon-blue-deep": "Colour-icon slot {blue-deep}: document fold, depth faces.",
    "icon-green": "Colour-icon slot {green}: spreadsheets, images, sync, shield. Omarchy `green`.",
    "icon-red": "Colour-icon slot {red}: PDF, video, full trash. Omarchy `red`.",
    "icon-orange": "Colour-icon slot {orange}: slides, star, alert. Omarchy `orange`.",
    "icon-purple": "Colour-icon slot {purple}: code, audio, vector, lock. Omarchy `magenta`.",
    "icon-yellow": "Colour-icon slot {yellow}: SD-card contacts.",
    "icon-slate": "Colour-icon slot {slate}: drives, config, neutral glyphs on paper.",
    "icon-slate-deep": "Colour-icon slot {slate-deep}: drive bases, terminal, script files.",
    "icon-paper": "Colour-icon slot {paper}: plain document page.",
    "icon-paper-fold": "Colour-icon slot {paper-fold}: page corner fold, placeholder lines.",
    "icon-on": "Colour-icon slot {on}: glyphs printed on coloured fills.",
}

def main():
    derived = {}
    for tid, tname, src in THEMES:
        p = ECHO if src is None else tomllib.loads((OMARCHY / src / "colors.toml").read_text())
        derived[tid] = derive(p)
        checks(tname, derived[tid])
    first = THEMES[0][0]
    color_names = [k for k in derived[first] if not k.startswith("shadow")]
    tokens = []
    for n in color_names:
        tokens.append({"name": n, "value": {tid: derived[tid][n] for tid, _, _ in THEMES}, "usage": USAGE[n]})
    for n, v in (("brand-ember", "#ff8a3d"), ("brand-violet", "#8b5cf6"), ("brand-sky", "#38bdf8"), ("brand-navy", "#0a1030")):
        tokens.append({"name": n, "value": v, "usage": USAGE[n]})
    doc = {
        "name": "EchoFiles", "version": 1,
        "meta": {"source": "Omarchy colors.toml (derived by design/build_tokens.py) + EchoFiles brand"},
        "color": {"themes": [{"id": tid, "name": tn} for tid, tn, _ in THEMES], "tokens": tokens},
        "type": {
            "fonts": [],
            "families": {"mono": "\"JetBrains Mono\", \"JetBrainsMono Nerd Font\", ui-monospace, monospace"},
            "groups": [
                {"name": "Display", "family": "mono", "styles": [
                    {"name": "display", "fontSize": "28px", "lineHeight": "34px", "fontWeight": 800, "letterSpacing": "-0.02em", "sample": "Nothing here yet", "usage": "Empty states and the About screen. One per view."},
                    {"name": "title", "fontSize": "16px", "lineHeight": "22px", "fontWeight": 700, "letterSpacing": "-0.01em", "sample": "Delete 3 items permanently?", "usage": "Dialog titles, preview-pane filename, drive name in Properties."}]},
                {"name": "Interface", "family": "mono", "styles": [
                    {"name": "body", "fontSize": "13px", "lineHeight": "20px", "fontWeight": 400, "sample": "IMG_20260914_182233.jpg", "usage": "Filenames, menu items, buttons, fields. The default. Omarchy font base-size + 1."},
                    {"name": "body-strong", "fontSize": "13px", "lineHeight": "20px", "fontWeight": 600, "sample": "Windows (C:)", "usage": "Active sidebar item, focused filename, emphasised values."},
                    {"name": "meta", "fontSize": "12px", "lineHeight": "16px", "fontWeight": 400, "sample": "4.2 MB · 14 Sep 18:22", "usage": "Size / date / kind columns, status bar, tile subtitles. Tabular figures."},
                    {"name": "label", "fontSize": "11px", "lineHeight": "16px", "fontWeight": 700, "letterSpacing": "0.08em", "sample": "WINDOWS", "usage": "Sidebar section headers and column headers. Uppercase."},
                    {"name": "caption", "fontSize": "11px", "lineHeight": "14px", "fontWeight": 500, "sample": "Read-only", "usage": "Pills, badges, key hints, tooltips."}]},
            ],
        },
        "spacing": {"tokens": [
            {"name": "space-0", "value": "2px", "usage": "Icon-to-badge nudge, pill vertical padding."},
            {"name": "space-1", "value": "4px", "usage": "Gap inside a control between icon and label; tile grid inner inset."},
            {"name": "space-2", "value": "6px", "usage": "Row horizontal padding in compact density; menu item inset."},
            {"name": "space-3", "value": "8px", "usage": "Default gap between sibling controls; row horizontal padding."},
            {"name": "space-4", "value": "12px", "usage": "Toolbar and sidebar padding; gap between tiles."},
            {"name": "space-5", "value": "16px", "usage": "Dialog and pane padding; toast padding."},
            {"name": "space-6", "value": "24px", "usage": "Gap between dialog sections; empty-state stack."},
            {"name": "space-7", "value": "32px", "usage": "Empty-state and About screen outer spacing."}]},
        "radius": {"tokens": [
            {"name": "radius-0", "value": "0px", "usage": "Window, panes, tabs, and every floating surface (menus, dialogs, toasts, palette, tooltips) — follows Hyprland `rounding` (Omarchy default 0)."},
            {"name": "radius-1", "value": "2px", "usage": "Rows, selection, menu items, pills, key hints, checkboxes, progress bars."},
            {"name": "radius-2", "value": "4px", "usage": "Buttons, fields, segmented controls, grid tiles."},
            {"name": "radius-3", "value": "6px", "usage": "Thumbnails and image previews only."},
            {"name": "radius-full", "value": "999px", "usage": "Switch track and thumb, status dots only."}]},
        "shadow": {"note": "Omarchy is flat: borders separate, shadows only lift what floats.", "tokens": [
            {"name": "shadow-pop", "value": {tid: derived[tid]["shadow-pop"] for tid, _, _ in THEMES}, "usage": "Menus, tooltips, drag ghost."},
            {"name": "shadow-float", "value": {tid: derived[tid]["shadow-float"] for tid, _, _ in THEMES}, "usage": "Dialogs, command palette, toasts."}]},
        "size": {"note": "Fixed geometry the virtualised list depends on. Rows never change height at runtime.", "tokens": [
            {"name": "row-compact", "value": "24px", "usage": "List row, compact density."},
            {"name": "row", "value": "28px", "usage": "List row, default density; menu item; sidebar item."},
            {"name": "row-comfy", "value": "34px", "usage": "List row, comfortable density."},
            {"name": "control", "value": "28px", "usage": "Button, field, segmented control height."},
            {"name": "toolbar", "value": "40px", "usage": "Toolbar height."},
            {"name": "tabbar", "value": "32px", "usage": "Tab strip height."},
            {"name": "statusbar", "value": "24px", "usage": "Status bar height."},
            {"name": "sidebar-width", "value": "236px", "usage": "Sidebar default width (resizable 180–360)."},
            {"name": "icon-glyph", "value": "16px", "usage": "UI glyphs in toolbar, menus, rows."},
            {"name": "icon-row", "value": "18px", "usage": "Colour file icon in list rows."},
            {"name": "icon-tile", "value": "48px", "usage": "Colour icon in grid tiles."},
            {"name": "tile", "value": "104px", "usage": "Grid tile width (icon 48 or thumbnail 88)."},
            {"name": "thumb", "value": "88px", "usage": "Thumbnail box inside a tile; decoded from the 128px freedesktop cache."}]},
        "duration": {"note": "Animations never delay input and are interruptible. prefers-reduced-motion (and Omarchy animations off) sets every duration to 0.", "tokens": [
            {"name": "dur-instant", "value": "0ms", "usage": "Keyboard cursor moves, selection changes, sort, typing."},
            {"name": "dur-fast", "value": "90ms", "usage": "Hover and press layers, checkbox tick, icon swaps."},
            {"name": "dur-base", "value": "140ms", "usage": "Menus, tooltips, popovers, pane toggles."},
            {"name": "dur-slow", "value": "220ms", "usage": "Dialogs, toasts, command palette, preview pane slide."},
            {"name": "dur-ambient", "value": "1400ms", "usage": "Skeleton shimmer and indeterminate progress sweep (one loop)."}]},
        "easing": {"tokens": [
            {"name": "ease-standard", "value": "cubic-bezier(0.2, 0, 0, 1)", "usage": "Anything entering or changing size."},
            {"name": "ease-exit", "value": "cubic-bezier(0.4, 0, 1, 1)", "usage": "Anything leaving; runs at 70% of the enter duration."},
            {"name": "ease-spring", "value": "cubic-bezier(0.34, 1.36, 0.64, 1)", "usage": "Toast arrival and drop-target pop only."},
            {"name": "ease-linear", "value": "linear", "usage": "Progress and shimmer."}]},
        "zIndex": {"tokens": [
            {"name": "z-sticky", "value": "10", "usage": "Column header, sticky breadcrumb."},
            {"name": "z-menu", "value": "30", "usage": "Context menus, popovers, tooltips."},
            {"name": "z-toast", "value": "40", "usage": "Toast stack."},
            {"name": "z-dialog", "value": "50", "usage": "Dialogs and their scrim."},
            {"name": "z-palette", "value": "60", "usage": "Command palette."}]},
        "opacity": {"tokens": [
            {"name": "opacity-disabled", "value": "0.45", "usage": "Disabled controls and unavailable menu items."},
            {"name": "opacity-hidden-file", "value": "0.6", "usage": "Dotfiles and Windows Hidden/System files when shown."},
            {"name": "opacity-cut", "value": "0.5", "usage": "Items on the clipboard after Cut."}]},
    }
    out = json.dumps(doc, indent=2, ensure_ascii=False)
    (ROOT / "design/system/project/tokens.json").write_text(out)
    (ROOT / "design/tokens.json").write_text(out)
    print(f"{len(tokens)} colour tokens x {len(THEMES)} themes")

if __name__ == "__main__":
    main()
