#!/usr/bin/env python3
"""EchoFiles icon source of truth.

Generates:
  assets/icons/glyph/<name>.svg   24px single-ink UI glyphs (stroke = currentColor)
  assets/icons/color/<name>.svg   48px full-colour place / device / file-type / status icons
  design/system/uploads/...       copies with a concrete ink for the design-system asset store
  design/system/icons.generated.js  icon data for the component bundle (colours as CSS vars)

Colour icons are authored against named palette slots ({folder}, {blue}, ...). The app
recolours them from the active Omarchy theme by substituting the slot hexes (see
design/build_tokens.py for the slot -> theme mapping).
"""
import json, math, pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
GLYPH_DIR = ROOT / "assets/icons/glyph"
COLOR_DIR = ROOT / "assets/icons/color"
UP = ROOT / "design/system/uploads"

# Default ("Echo") slot values, sampled from the reference icon sheets.
SLOTS = {
    "folder": "#FFC83D", "folder-back": "#F2AE1C", "folder-glyph": "#7A5306",
    "blue": "#2F7CF6", "blue-deep": "#1B5FD1", "green": "#22A55B", "red": "#EF4444",
    "orange": "#FB8A2E", "purple": "#7B4FF0", "yellow": "#F7C331",
    "slate": "#4B5565", "slate-deep": "#2E3440", "paper": "#E6EBF2",
    "paper-fold": "#BFC8D6", "on": "#FFFFFF",
}
GLYPH_INK = "#4B5565"

# ---------------------------------------------------------------- glyphs (24 grid)
def gear_path(cx=12, cy=12, ro=9.2, ri=7.0, teeth=8):
    pts = []
    step = 2 * math.pi / teeth
    for i in range(teeth):
        a = i * step - math.pi / 2
        for da, r in ((-0.30, ri), (-0.17, ro), (0.17, ro), (0.30, ri)):
            pts.append((cx + r * math.cos(a + da * step / 0.6 * 0.6), cy + r * math.sin(a + da * step / 0.6 * 0.6)))
    d = "M" + " L".join(f"{x:.2f} {y:.2f}" for x, y in pts) + "Z"
    return d

GEAR = f'<path d="{gear_path()}"/><circle cx="12" cy="12" r="3"/>'
FOLDER_P = "M3.5 7a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"
FILE_P = "M14 3.5H7a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8.5zM14 3.5v5h5"
EYE_P = "M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z"

GLYPHS = {
    # navigation
    "arrow-left": '<path d="M19 12H5M11 6l-6 6 6 6"/>',
    "arrow-right": '<path d="M5 12h14M13 6l6 6-6 6"/>',
    "arrow-up": '<path d="M12 19V5M6 11l6-6 6 6"/>',
    "arrow-down": '<path d="M12 5v14M6 13l6 6 6-6"/>',
    "chevron-left": '<path d="M15 6l-6 6 6 6"/>',
    "chevron-right": '<path d="M9 6l6 6-6 6"/>',
    "chevron-up": '<path d="M6 15l6-6 6 6"/>',
    "chevron-down": '<path d="M6 9l6 6 6-6"/>',
    "home": '<path d="M4 10.5L12 4l8 6.5V19a1 1 0 0 1-1 1h-4.5v-6h-5v6H5a1 1 0 0 1-1-1z"/>',
    "history": '<path d="M3.5 12a8.5 8.5 0 1 0 2.5-6L3.5 8.5M3.5 4v4.5H8M12 8v4.5l3 2"/>',
    # views
    "grid": '<rect x="4" y="4" width="6.5" height="6.5" rx="1.5"/><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.5"/><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.5"/><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.5"/>',
    "list": '<path d="M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01"/>',
    "columns": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M12 4v16"/>',
    "sidebar": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/>',
    "sort": '<path d="M7 4v16M3.5 7.5L7 4l3.5 3.5M17 20V4M13.5 16.5L17 20l3.5-3.5"/>',
    "filter": '<path d="M4 5h16l-6 7.5V19l-4-2v-4.5z"/>',
    "sliders": '<path d="M4 7h9M17 7h3M4 17h3M11 17h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/>',
    "more-vertical": '<path d="M12 5h.01M12 12h.01M12 19h.01" stroke-width="3"/>',
    "more-horizontal": '<path d="M5 12h.01M12 12h.01M19 12h.01" stroke-width="3"/>',
    "search": '<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>',
    "eye": f'<path d="{EYE_P}"/><circle cx="12" cy="12" r="3"/>',
    "eye-off": f'<path d="{EYE_P}"/><circle cx="12" cy="12" r="3"/><path d="M4 4l16 16"/>',
    # edit
    "copy": '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>',
    "cut": '<circle cx="6.5" cy="17.5" r="2.5"/><circle cx="17.5" cy="17.5" r="2.5"/><path d="M8.3 15.7L18 4M15.7 15.7L6 4"/>',
    "paste": '<rect x="5" y="5" width="14" height="16" rx="2"/><path d="M9 5V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v1M9 11h6M9 15h4"/>',
    "rename": '<path d="M4 20l1-4.5L15.5 5a2.1 2.1 0 0 1 3 3L8 18.5zM13.5 7l3 3"/>',
    "trash": '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/>',
    "undo": '<path d="M9 14l-5-5 5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
    "redo": '<path d="M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 0 0 0 11H13"/>',
    "plus": '<path d="M12 5v14M5 12h14"/>',
    "minus": '<path d="M5 12h14"/>',
    "close": '<path d="M6 6l12 12M18 6L6 18"/>',
    "check": '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    "folder": f'<path d="{FOLDER_P}"/>',
    "folder-plus": f'<path d="{FOLDER_P}"/><path d="M12 10.5v5M9.5 13h5"/>',
    "file": f'<path d="{FILE_P}"/>',
    "file-plus": f'<path d="{FILE_P}"/><path d="M12 11.5v6M9 14.5h6"/>',
    # transfer
    "download": '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
    "upload": '<path d="M12 16V5M7 10l5-5 5 5M5 20h14"/>',
    "move": '<path d="M14 6l5 5-5 5M19 11h-8a6 6 0 0 0-6 6"/>',
    "swap": '<path d="M4 8h15M15 4l4 4-4 4M20 16H5M9 12l-4 4 4 4"/>',
    "merge": '<path d="M12 4v16M3 12h6M6.5 9.5L9 12l-2.5 2.5M21 12h-6M17.5 9.5L15 12l2.5 2.5"/>',
    "compress": '<path d="M4 4l5 5M9 5v4H5M20 4l-5 5M15 5v4h4M4 20l5-5M9 19v-4H5M20 20l-5-5M15 19v-4h4"/>',
    "external": '<path d="M14 4h6v6M20 4l-9 9M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4"/>',
    "link": '<path d="M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1"/>',
    "share": '<circle cx="18" cy="5.5" r="2.5"/><circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="18.5" r="2.5"/><path d="M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1"/>',
    "send": '<path d="M21 3L10 14M21 3l-6.5 18-4.5-7-7-4.5z"/>',
    "sync": '<path d="M20 11a8 8 0 0 0-14.3-4.3L4 8.5M4 4v4.5h4.5M4 13a8 8 0 0 0 14.3 4.3l1.7-1.8M20 20v-4.5h-4.5"/>',
    "refresh": '<path d="M20 12a8 8 0 1 1-2.34-5.66M20 4v5h-5"/>',
    "cloud": '<path d="M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5z"/>',
    "cloud-upload": '<path d="M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5H15"/><path d="M12 20v-7M9.5 15.5L12 13l2.5 2.5M9 18H7"/>',
    "cloud-download": '<path d="M7 18a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5H15M9 18H7"/><path d="M12 12v8M9.5 17.5L12 20l2.5-2.5"/>',
    # status & meta
    "star": '<path d="M12 3.5l2.6 5.3 5.9.9-4.25 4.1 1 5.8L12 16.9l-5.25 2.7 1-5.8L3.5 9.7l5.9-.9z"/>',
    "tag": '<path d="M3.5 12.3V4.5a1 1 0 0 1 1-1h7.8a2 2 0 0 1 1.4.6l7.2 7.2a2 2 0 0 1 0 2.8l-6.6 6.6a2 2 0 0 1-2.8 0l-7.2-7.2a2 2 0 0 1-.6-1.4z"/><path d="M8 8h.01" stroke-width="3"/>',
    "pin": '<path d="M9 4h6l-1 5 3 3v2H7v-2l3-3zM12 14v6"/>',
    "info": '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 7.5h.01"/>',
    "alert": '<path d="M12 3.5l9.5 16.5h-19zM12 10v4M12 17h.01"/>',
    "error": '<circle cx="12" cy="12" r="9"/><path d="M12 7.5v5M12 16h.01"/>',
    "clock": '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    "lock": '<rect x="5" y="10.5" width="14" height="10" rx="2"/><path d="M8 10.5V7.5a4 4 0 0 1 8 0v3M12 14.5v2"/>',
    "unlock": '<rect x="5" y="10.5" width="14" height="10" rx="2"/><path d="M8 10.5V7.5a4 4 0 0 1 7.6-1.7M12 14.5v2"/>',
    "key": '<circle cx="8" cy="15" r="4.5"/><path d="M11.2 11.8L20 3M16.5 6.5l2.5 2.5M14 9l2 2"/>',
    "shield": '<path d="M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6z"/><path d="M8.5 12l2.5 2.5 4.5-4.5"/>',
    "heart": '<path d="M12 20s-7.5-4.6-7.5-10A4.3 4.3 0 0 1 12 7.3 4.3 4.3 0 0 1 19.5 10c0 5.4-7.5 10-7.5 10z"/>',
    "user": '<circle cx="12" cy="8" r="3.5"/><path d="M5 20a7 7 0 0 1 14 0"/>',
    "users": '<circle cx="9" cy="8.5" r="3"/><path d="M3.5 19.5a5.5 5.5 0 0 1 11 0M15.5 5.8a3 3 0 0 1 0 5.4M17 14.2a5.5 5.5 0 0 1 3.5 5.3"/>',
    # system
    "settings": GEAR,
    "terminal": '<path d="M5 7l5 5-5 5M12 18h7"/>',
    "command": '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7 10l2.5 2L7 14M12 14h5"/>',
    "keyboard": '<rect x="2.5" y="6" width="19" height="12" rx="2"/><path d="M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M8 14h8"/>',
    "drive": '<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M3 13h18M16.5 15.5h.01M13.5 15.5h.01"/>',
    "usb": '<path d="M8 10h8v9a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2zM9.5 10V3.5h5V10M11 6.5h.01M13 6.5h.01"/>',
    "sd-card": '<path d="M8 3h8.5L19 5.5V20a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6zM9 7v3M12 7v3M15 7v3"/>',
    "phone": '<rect x="6.5" y="2.5" width="11" height="19" rx="2.5"/><path d="M10.5 18.5h3"/>',
    "network": '<rect x="9" y="3" width="6" height="5" rx="1"/><rect x="3" y="16" width="6" height="5" rx="1"/><rect x="15" y="16" width="6" height="5" rx="1"/><path d="M12 8v4M6 16v-2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v2"/>',
    "server": '<rect x="3.5" y="4" width="17" height="7" rx="1.5"/><rect x="3.5" y="13" width="17" height="7" rx="1.5"/><path d="M7 7.5h.01M7 16.5h.01"/>',
    # media
    "image": '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8.5" cy="9.5" r="1.5"/><path d="M21 16l-5-5-9 9"/>',
    "music": '<path d="M9 18V5.5l11-2V16"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="17.5" cy="16" r="2.5"/>',
    "video": '<rect x="3" y="5" width="13" height="14" rx="2"/><path d="M16 10l5-3v10l-5-3"/>',
    "play": '<path d="M7 4.5v15l12-7.5z"/>',
    "pause": '<path d="M9 5v14M15 5v14"/>',
    "cancel": '<circle cx="12" cy="12" r="9"/><path d="M9 9l6 6M15 9l-6 6"/>',
    "crop": '<path d="M6 2v14a2 2 0 0 0 2 2h14M2 6h14a2 2 0 0 1 2 2v14"/>',
    "rotate": '<path d="M20 12a8 8 0 1 1-2.34-5.66M20 4v5h-5"/>',
    "archive": '<rect x="3" y="4" width="18" height="5" rx="1"/><path d="M5 9v9a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9M10 13h4"/>',
    "code": '<path d="M8 7l-5 5 5 5M16 7l5 5-5 5M13.5 4.5l-3 15"/>',
}

def glyph_svg(name, ink="currentColor"):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" '
            f'fill="none" stroke="{ink}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">'
            f'{GLYPHS[name]}</svg>\n')

# ---------------------------------------------------------------- colour icons (48 grid)
def g(name, color, x, y, s, sw=2.4, fill="none"):
    """Place a 24-grid glyph inside a 48 icon at (x,y) with size s."""
    k = s / 24
    return (f'<g transform="translate({x} {y}) scale({k:.4f})" fill="{fill}" stroke="{color}" '
            f'stroke-width="{sw / k:.2f}" stroke-linecap="round" stroke-linejoin="round">{GLYPHS[name]}</g>')

FOLDER_BACK = '<path fill="{folder-back}" d="M4 12a3 3 0 0 1 3-3h10.6a3 3 0 0 1 2.1.9L23 13h18a3 3 0 0 1 3 3v21a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z"/>'
FOLDER_FRONT = '<path fill="{folder}" d="M4 19a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3z"/>'

def folder(glyph=None, filled=False):
    body = FOLDER_BACK + FOLDER_FRONT
    if glyph:
        body += g(glyph, "{folder-glyph}", 16, 20, 16, 2.6, "{folder-glyph}" if filled else "none")
    return body

def folder_open():
    return (FOLDER_BACK +
            '<path fill="{folder}" d="M9.3 18h35.2a2 2 0 0 1 1.9 2.7l-5.5 16.5A4 4 0 0 1 37.1 40H6a2 2 0 0 1-1.9-2.7l5.3-17.4A2 2 0 0 1 9.3 18z"/>')

def page(fill, fold, glyph_markup=""):
    return (f'<path fill="{fill}" d="M12 4h17l11 11v26a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z"/>'
            f'<path fill="{fold}" d="M29 4l11 11h-8a3 3 0 0 1-3-3z"/>' + glyph_markup)

def light_page(tint, glyph_markup):
    # tinted paper: paper mixed toward a type colour; resolved per theme by the bundle / app
    return page("{paper}", "{paper-fold}", f'<rect x="9" y="4" width="31" height="40" rx="3" fill="{tint}" opacity="0.10"/>' + glyph_markup)

def lines(color, x=15, y=22, w=19, n=3, gap=5):
    return "".join(f'<rect x="{x}" y="{y + i * gap}" width="{w if i < n - 1 else w - 6}" height="2.6" rx="1.3" fill="{color}"/>' for i in range(n))

def badge(color, glyph):
    return (f'<circle cx="36" cy="36" r="9" fill="{color}"/><circle cx="36" cy="36" r="9" fill="none" stroke="{{on}}" stroke-width="2"/>'
            + g(glyph, "{on}", 30, 30, 12, 2.6))

A_LETTER = '<path fill="{slate}" d="M14.5 34l5-14h3l5 14h-3.1l-1.1-3.3h-4.6L17.6 34zm5-5.9h3l-1.5-4.6z"/><path fill="{slate}" d="M31.6 34.2c-2 0-3.3-1.2-3.3-3 0-2 1.5-3 4.3-3.1l1.9-.1v-.4c0-.9-.6-1.4-1.7-1.4-1 0-1.6.4-1.8 1.1h-2.6c.2-2.1 2-3.4 4.5-3.4 2.7 0 4.3 1.3 4.3 3.6V34h-2.6v-1.3c-.5.9-1.6 1.5-3 1.5zm.9-2.1c1.1 0 2-.7 2-1.7v-.6l-1.6.1c-1 .1-1.5.5-1.5 1.1s.4 1.1 1.1 1.1z"/>'
T_LETTER = '<path fill="{slate}" d="M16 20h17v3.4h-6.8V36h-3.4V23.4H16z"/>'

COLOR = {
    # places
    "folder": folder(),
    "folder-open": folder_open(),
    "folder-plus": folder("plus"),
    "folder-minus": folder("minus"),
    "folder-up": folder("arrow-up"),
    "folder-download": folder("download"),
    "folder-link": folder("link"),
    "folder-heart": folder("heart", True),
    "folder-lock": FOLDER_BACK + FOLDER_FRONT + '<rect x="18" y="26" width="12" height="9.5" rx="2" fill="{folder-glyph}"/><path d="M20.5 26v-2.5a3.5 3.5 0 0 1 7 0V26" fill="none" stroke="{folder-glyph}" stroke-width="2.4"/>',
    "folder-clock": folder("clock"),
    "folder-move": folder("move"),
    "folder-share": folder("share"),
    "folder-users": folder("users"),
    "folder-user": folder("user"),
    "folder-settings": folder("settings"),
    "folder-search": folder("search"),
    "folder-image": folder("image"),
    "folder-music": folder("music"),
    "folder-video": FOLDER_BACK + FOLDER_FRONT + '<path d="M21 23.5v11l9-5.5z" fill="{folder-glyph}"/>',
    "home": '<path fill="{blue}" d="M24 5.5l18 15.2V40a3 3 0 0 1-3 3H29.5V31h-11v12H9a3 3 0 0 1-3-3V20.7z"/><path fill="{blue-deep}" d="M24 5.5l18 15.2v3.1L24 8.6 6 23.8v-3.1z"/>',
    "trash": '<rect x="15" y="4" width="18" height="6" rx="2" fill="{slate}"/><rect x="7" y="8" width="34" height="6" rx="2" fill="{slate}"/><path fill="{paper-fold}" d="M10 16h28l-2.2 23.3A4 4 0 0 1 31.8 43H16.2a4 4 0 0 1-4-3.7z"/><path d="M20 22v14M28 22v14" stroke="{slate}" stroke-width="3" stroke-linecap="round"/>',
    "trash-full": '<rect x="15" y="4" width="18" height="6" rx="2" fill="{red}"/><rect x="7" y="8" width="34" height="6" rx="2" fill="{red}"/><path fill="{red}" d="M10 16h28l-2.2 23.3A4 4 0 0 1 31.8 43H16.2a4 4 0 0 1-4-3.7z"/><path d="M20 22v14M28 22v14" stroke="{on}" stroke-width="3" stroke-linecap="round" opacity="0.9"/>',
    # devices
    "drive": '<path fill="{slate}" d="M11.5 9h25a3 3 0 0 1 2.9 2.3L43 27H5l3.6-15.7A3 3 0 0 1 11.5 9z"/><rect x="5" y="26" width="38" height="13" rx="3" fill="{slate-deep}"/><circle cx="36" cy="32.5" r="2" fill="{on}"/><circle cx="30" cy="32.5" r="2" fill="{on}" opacity="0.7"/>',
    "drive-external": '<rect x="5" y="12" width="38" height="24" rx="5" fill="{slate-deep}"/><rect x="5" y="12" width="38" height="12" rx="5" fill="{slate}"/><rect x="5" y="20" width="38" height="4" fill="{slate}"/><circle cx="36" cy="30" r="2" fill="{on}"/>',
    "server": '<rect x="6" y="6" width="36" height="16" rx="3" fill="{slate}"/><rect x="6" y="26" width="36" height="16" rx="3" fill="{slate-deep}"/><circle cx="34" cy="14" r="2" fill="{on}"/><circle cx="34" cy="34" r="2" fill="{on}"/><rect x="12" y="12.7" width="12" height="2.6" rx="1.3" fill="{on}" opacity="0.6"/>',
    "usb": '<rect x="16" y="4" width="16" height="14" rx="2" fill="{paper-fold}"/><rect x="20" y="8" width="3" height="4" rx="1" fill="{slate}"/><rect x="25" y="8" width="3" height="4" rx="1" fill="{slate}"/><rect x="12" y="16" width="24" height="28" rx="5" fill="{slate}"/><rect x="20" y="34" width="8" height="3" rx="1.5" fill="{paper-fold}"/>',
    "sd-card": '<path fill="{slate-deep}" d="M15 4h19l6 6v30a4 4 0 0 1-4 4H12a4 4 0 0 1-4-4V11z"/>' + "".join(f'<rect x="{x}" y="9" width="3.4" height="10" rx="1" fill="{{yellow}}"/>' for x in (16, 21, 26, 31)),
    "phone": '<rect x="12" y="3" width="24" height="42" rx="6" fill="{blue}"/><rect x="15" y="7" width="18" height="31" rx="2.5" fill="{on}" opacity="0.92"/><rect x="20" y="40.2" width="8" height="2.2" rx="1.1" fill="{on}"/>',
    "network": '<rect x="17" y="4" width="14" height="11" rx="2" fill="{blue}"/><rect x="5" y="33" width="14" height="11" rx="2" fill="{blue}"/><rect x="29" y="33" width="14" height="11" rx="2" fill="{blue}"/><path d="M24 15v8M12 33v-4a3 3 0 0 1 3-3h18a3 3 0 0 1 3 3v4" fill="none" stroke="{blue-deep}" stroke-width="3" stroke-linecap="round"/>',
    "cloud": '<path fill="{blue}" d="M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z"/>',
    "cloud-upload": '<path fill="{blue}" d="M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z"/>' + g("arrow-up", "{on}", 16, 17, 16, 3),
    "cloud-download": '<path fill="{blue}" d="M14 38a9 9 0 0 1-1.3-17.9A12 12 0 0 1 36 19a8.5 8.5 0 0 1-1 19z"/>' + g("arrow-down", "{on}", 16, 18, 16, 3),
    # files
    "file": page("{paper}", "{paper-fold}", lines("{paper-fold}")),
    "file-text": page("{paper}", "{paper-fold}", lines("{blue}")),
    "file-doc": page("{blue}", "{blue-deep}", lines("{on}")),
    "file-sheet": page("{green}", "{on}", '<rect x="15" y="21" width="19" height="15" rx="1.5" fill="none" stroke="{on}" stroke-width="2.6"/><path d="M15 28.5h19M24.5 21v15" stroke="{on}" stroke-width="2.6"/>'),
    "file-slides": page("{orange}", "{on}", '<rect x="15" y="22" width="19" height="12" rx="1.5" fill="none" stroke="{on}" stroke-width="2.6"/><rect x="18.5" y="26.5" width="12" height="3" rx="1" fill="{on}"/>'),
    "file-pdf": page("{red}", "{on}", '<rect x="15" y="19" width="12" height="2.6" rx="1.3" fill="{on}"/><rect x="15" y="24" width="19" height="2.6" rx="1.3" fill="{on}"/><rect x="13" y="30" width="23" height="9" rx="2" fill="{on}"/><path d="M16.5 37v-5h2a1.5 1.5 0 0 1 0 3h-2M22.5 37v-5h1.5a2.5 2.5 0 0 1 0 5zM29.5 37v-5h3M29.5 34.5h2.4" fill="none" stroke="{red}" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>'),
    "file-code": page("{purple}", "{on}", g("code", "{on}", 14.5, 20, 20, 2.8)),
    "file-config": page("{slate}", "{paper-fold}", g("settings", "{on}", 15, 20, 18, 2.6)),
    "file-font": page("{paper}", "{paper-fold}", A_LETTER),
    "file-type": page("{paper}", "{paper-fold}", T_LETTER),
    "file-lines": page("{paper}", "{paper-fold}", lines("{slate}", n=4, y=20)),
    "file-list": page("{paper}", "{paper-fold}", "".join(f'<circle cx="16.5" cy="{22.3 + i * 5}" r="1.6" fill="{{slate}}"/><rect x="20" y="{21 + i * 5}" width="14" height="2.6" rx="1.3" fill="{{slate}}"/>' for i in range(3))),
    "file-script": page("{slate-deep}", "{slate}", g("chevron-right", "{on}", 13, 21, 14, 3) + '<rect x="25" y="32" width="9" height="2.8" rx="1.4" fill="{on}"/>'),
    "file-image": light_page("{green}", '<circle cx="19" cy="23" r="3" fill="{orange}"/><path fill="{green}" d="M13 38l7.5-9 4.5 5 4-4.5 7 8.5z"/>'),
    "file-video": light_page("{red}", '<rect x="14" y="21" width="21" height="15" rx="2.5" fill="{red}"/><path d="M22 24.8v7.4l6-3.7z" fill="{on}"/>'),
    "file-audio": light_page("{purple}", g("music", "{purple}", 14, 19, 20, 2.8)),
    "file-vector": light_page("{purple}", '<path fill="{purple}" d="M13 38l7.5-10 4.5 6 3-4 6 8z"/>' + g("rename", "{purple}", 23, 17, 14, 2.6)),
    "file-archive": page("{folder}", "{folder-back}", "".join(f'<rect x="{21 if i % 2 else 24}" y="{6 + i * 3.4}" width="3" height="3" fill="{{folder-glyph}}"/>' for i in range(8)) + '<rect x="20.5" y="33" width="7" height="6" rx="1.5" fill="{folder-glyph}"/>'),
    "file-archive-alt": page("{slate}", "{slate-deep}", "".join(f'<rect x="{21 if i % 2 else 24}" y="{6 + i * 3.4}" width="3" height="3" fill="{{slate-deep}}"/>' for i in range(10)) + '<rect x="20.5" y="39" width="7" height="3" rx="1" fill="{slate-deep}"/>'),
    "file-package": '<rect x="6" y="7" width="36" height="11" rx="3" fill="{red}"/><rect x="6" y="18" width="36" height="11" rx="0" fill="{blue}"/><rect x="6" y="29" width="36" height="12" rx="3" fill="{green}"/><rect x="21" y="7" width="7" height="34" fill="{folder}"/><rect x="19.5" y="20.5" width="10" height="7" rx="1.5" fill="{folder-glyph}"/><rect x="36" y="10" width="2.4" height="5" rx="1" fill="{on}" opacity="0.8"/>',
    "file-verified": page("{paper}", "{paper-fold}") + badge("{blue}", "check"),
    "file-error": page("{paper}", "{paper-fold}") + f'<circle cx="36" cy="36" r="9" fill="{{red}}"/><circle cx="36" cy="36" r="9" fill="none" stroke="{{on}}" stroke-width="2"/><path d="M36 31v6" stroke="{{on}}" stroke-width="2.8" stroke-linecap="round"/><circle cx="36" cy="40.6" r="1.6" fill="{{on}}"/>',
    "file-new": page("{paper}", "{paper-fold}") + badge("{blue}", "plus"),
    # status
    "lock": '<path d="M16 21v-5a8 8 0 0 1 16 0v5" fill="none" stroke="{purple}" stroke-width="4.5"/><rect x="10" y="20" width="28" height="23" rx="5" fill="{purple}"/><circle cx="24" cy="30" r="3" fill="{on}"/><rect x="22.6" y="31" width="2.8" height="6" rx="1.4" fill="{on}"/>',
    "shield": '<path fill="{green}" d="M24 4l16 6v11.5C40 31.5 33.3 39.6 24 44 14.7 39.6 8 31.5 8 21.5V10z"/>' + g("check", "{on}", 14, 13, 20, 3.4),
    "key": g("key", "{slate}", 3, 3, 42, 4.2),
    "eye": '<path fill="{blue}" d="M3 24S11 11 24 11s21 13 21 13-8 13-21 13S3 24 3 24z"/><circle cx="24" cy="24" r="7.5" fill="{on}"/><circle cx="24" cy="24" r="4" fill="{blue-deep}"/>',
    "eye-off": '<path fill="{paper-fold}" d="M3 24S11 11 24 11s21 13 21 13-8 13-21 13S3 24 3 24z"/><circle cx="24" cy="24" r="7.5" fill="{slate}"/><path d="M9 9l30 30" stroke="{slate}" stroke-width="4.5" stroke-linecap="round"/>',
    "share": g("share", "{blue}", 3, 3, 42, 4.4),
    "send": '<path fill="{blue}" d="M43 5L4.5 20.5l14 6.5 6.5 14z"/><path fill="{blue-deep}" d="M43 5L18.5 27 25 41z"/>',
    "sync": g("sync", "{green}", 3, 3, 42, 4.4),
    "history": g("history", "{slate}", 3, 3, 42, 4.2),
    "star": '<path fill="{orange}" d="M24 4.5l5.8 11.8 13 1.9-9.4 9.2 2.2 12.9L24 34.2l-11.6 6.1 2.2-12.9-9.4-9.2 13-1.9z"/>',
    "tag": '<path fill="{blue}" d="M5 22.3V8a3 3 0 0 1 3-3h14.3a4 4 0 0 1 2.8 1.2l16.2 16.2a4 4 0 0 1 0 5.6L27.6 41.7a4 4 0 0 1-5.6 0L6.2 25.1A4 4 0 0 1 5 22.3z"/><circle cx="14.5" cy="14.5" r="3.5" fill="{on}"/>',
    "info": '<circle cx="24" cy="24" r="20" fill="{blue}"/><rect x="21.5" y="21" width="5" height="14" rx="2.5" fill="{on}"/><circle cx="24" cy="14.5" r="3" fill="{on}"/>',
    "settings": g("settings", "{slate}", 3, 3, 42, 4.2),
    "sliders": g("sliders", "{slate}", 3, 3, 42, 4.2),
    "terminal": '<rect x="4" y="6" width="40" height="36" rx="6" fill="{slate-deep}"/>' + g("chevron-right", "{on}", 9, 14, 20, 3.4) + '<rect x="25" y="29" width="11" height="3.4" rx="1.7" fill="{on}"/>',
    "alert": '<path fill="{orange}" d="M21.4 6.5a3 3 0 0 1 5.2 0l17 29.5a3 3 0 0 1-2.6 4.5H7a3 3 0 0 1-2.6-4.5z"/><rect x="21.6" y="16" width="4.8" height="13" rx="2.4" fill="{on}"/><circle cx="24" cy="34" r="2.6" fill="{on}"/>',
}

def resolve(markup, slots):
    for k in sorted(slots, key=len, reverse=True):
        markup = markup.replace("{" + k + "}", slots[k])
    return markup

def color_svg(name, slots=SLOTS):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48" width="48" height="48">'
            f'{resolve(COLOR[name], slots)}</svg>\n')

LOGO = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <defs>
    <radialGradient id="ef-tile" cx="0.5" cy="0.35" r="0.75">
      <stop offset="0" stop-color="#101d5c"/><stop offset="1" stop-color="#050a24"/>
    </radialGradient>
    <linearGradient id="ef-back" x1="0" y1="0" x2="0.3" y2="1">
      <stop offset="0" stop-color="#ffb84d"/><stop offset="0.55" stop-color="#ff7a3d"/><stop offset="1" stop-color="#f04a6a"/>
    </linearGradient>
    <linearGradient id="ef-mid" x1="0" y1="0" x2="0.4" y2="1">
      <stop offset="0" stop-color="#c38bff"/><stop offset="1" stop-color="#5b3cf0"/>
    </linearGradient>
    <linearGradient id="ef-front" x1="0.9" y1="0" x2="0.1" y2="1">
      <stop offset="0" stop-color="#46e4ff"/><stop offset="0.5" stop-color="#4aa8ff"/><stop offset="1" stop-color="#2f6bff"/>
    </linearGradient>
    <filter id="ef-glow" x="-30%" y="-30%" width="160%" height="160%">
      <feGaussianBlur stdDeviation="5" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>
  </defs>
  <rect x="20" y="20" width="216" height="216" rx="54" fill="url(#ef-tile)" stroke="#1e3ea8" stroke-width="2"/>
  <g filter="url(#ef-glow)">
    <path fill="url(#ef-back)" d="M62 78c0-8 5-14 13-16.5l40-11.5c9-2.6 16 2.5 18 10l6 20v88L64 180c-1.5-1-2-3-2-5z"/>
    <path fill="url(#ef-mid)" opacity="0.96" d="M84 94c0-8 5-14 13-16.4l43-12.3c9-2.5 15.5 2 17.5 9.2l5.5 18.5v84L90 195c-3 .8-6-1.5-6-4.6z"/>
    <path fill="url(#ef-front)" d="M108 116c0-9 5.5-15.5 14-18l23-6.6c4-1.1 7.2.4 9 3.5l2.5 4.2 30-8.6c9.5-2.7 17.5 3.4 17.5 13.2v6.5c0 9-5.5 15.5-14 18l-44 12.6c-6 1.7-9.5 6.6-9.5 12.6v10l37-10.6c8.8-2.5 15.5 2.4 15.5 11v2.5c0 8.5-4.8 14.6-12.5 17.8-18 7.5-26 13-34 23-5 6-10 8.5-15 8.5-7 0-9.5-5-9.5-11z"/>
  </g>
</svg>
"""

def main():
    GLYPH_DIR.mkdir(parents=True, exist_ok=True)
    COLOR_DIR.mkdir(parents=True, exist_ok=True)
    (UP / "Glyphs").mkdir(parents=True, exist_ok=True)
    (UP / "Icons").mkdir(parents=True, exist_ok=True)
    (UP / "Logos").mkdir(parents=True, exist_ok=True)
    for n in GLYPHS:
        (GLYPH_DIR / f"{n}.svg").write_text(glyph_svg(n))
        (UP / "Glyphs" / f"{n}.svg").write_text(glyph_svg(n, GLYPH_INK))
    for n in COLOR:
        (COLOR_DIR / f"{n}.svg").write_text(color_svg(n))
        (UP / "Icons" / f"{n}.svg").write_text(color_svg(n))
    (ROOT / "assets/brand/echofiles-logo.svg").write_text(LOGO)
    (UP / "Logos" / "echofiles-logo.svg").write_text(LOGO)
    # bundle data: colour slots become CSS variables so icons follow the theme
    css_slots = {k: f"var(--icon-{k})" for k in SLOTS}
    data = {
        "glyphs": GLYPHS,
        "color": {n: resolve(m, css_slots) for n, m in COLOR.items()},
    }
    (ROOT / "design/system/icons.generated.js").write_text(
        "var ICONS = " + json.dumps(data, separators=(",", ":")) + ";\n")
    json.dump({"slots": SLOTS, "glyphs": sorted(GLYPHS), "color": sorted(COLOR)},
              open(ROOT / "design/icons.json", "w"), indent=2)
    print(f"{len(GLYPHS)} glyphs, {len(COLOR)} colour icons")

if __name__ == "__main__":
    main()
