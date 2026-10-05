(function () {
  var React = window.React;
  var h = React.createElement;
  var useState = React.useState;

  function cx() {
    var out = [];
    for (var i = 0; i < arguments.length; i++) if (arguments[i]) out.push(arguments[i]);
    return out.join(" ");
  }
  function rest(props, omit) {
    var o = {};
    for (var k in props) if (omit.indexOf(k) < 0) o[k] = props[k];
    return o;
  }
  var SYS_SANS = "Inter, 'SamsungOne', 'Google Sans', Roboto, system-ui, sans-serif";
  var LAPTOP = "Aditya's laptop";

  // ------------------------------------------------------------ icons (shared with EchoFiles)
  function Icon(p) {
    var size = p.size || 20;
    return h("svg", {
      className: cx("ec-icon", p.className), width: size, height: size, viewBox: "0 0 24 24",
      fill: "none", stroke: "currentColor", strokeWidth: p.strokeWidth || 2, strokeLinecap: "round",
      strokeLinejoin: "round", "aria-hidden": p.label ? undefined : "true", role: p.label ? "img" : undefined,
      "aria-label": p.label, style: p.style, dangerouslySetInnerHTML: { __html: ICONS.glyphs[p.name] || "" }
    });
  }

  var EXT = {
    jpg: "file-image", jpeg: "file-image", png: "file-image", webp: "file-image", heic: "file-image",
    mp4: "file-video", mkv: "file-video", mov: "file-video", mp3: "file-audio", m4a: "file-audio", opus: "file-audio",
    pdf: "file-pdf", doc: "file-doc", docx: "file-doc", txt: "file-lines", md: "file-text",
    xls: "file-sheet", xlsx: "file-sheet", csv: "file-sheet", ppt: "file-slides", pptx: "file-slides",
    zip: "file-archive", apk: "file-package", rs: "file-code", py: "file-code", sh: "file-script"
  };
  function iconFor(name, kind) {
    if (kind === "folder") return "folder";
    var m = /\.([^.]+)$/.exec(name || "");
    return (m && EXT[m[1].toLowerCase()]) || "file";
  }
  function FileIcon(p) {
    var size = p.size || 32;
    return h("span", { className: cx("ec-ficon", p.className), style: { width: size, height: size }, "aria-hidden": "true" },
      h("svg", { viewBox: "0 0 48 48", dangerouslySetInnerHTML: { __html: ICONS.color[p.name] || ICONS.color.file } }));
  }

  // ------------------------------------------------------------ actions
  function Button(p) {
    var o = rest(p, ["variant", "size", "icon", "block", "className", "children"]);
    o.className = cx("ec-btn", p.variant && "ec-btn-" + p.variant, p.size && "ec-btn-" + p.size, p.block && "ec-btn-block", p.className);
    o.type = o.type || "button";
    return h("button", o, p.icon ? h(Icon, { name: p.icon, size: p.size === "sm" ? 16 : 18 }) : null, p.children);
  }

  function IconButton(p) {
    var o = rest(p, ["icon", "label", "pressed", "className", "size"]);
    o.className = cx("ec-ibtn", p.className);
    o.type = "button";
    o["aria-label"] = p.label;
    o.title = p.label;
    if (p.pressed !== undefined) o["aria-pressed"] = String(!!p.pressed);
    return h("button", o, h(Icon, { name: p.icon, size: p.size || 22 }));
  }

  function Switch(p) {
    var st = useState(!!p.defaultChecked);
    var on = p.checked !== undefined ? p.checked : st[0];
    return h("button", {
      type: "button", role: "switch", "aria-checked": String(on), className: "ec-switch", id: p.id, disabled: p.disabled,
      "aria-label": p.label, onClick: function (e) { e.stopPropagation(); st[1](!on); p.onChange && p.onChange(!on); }
    });
  }

  function Checkbox(p) {
    var st = useState(!!p.defaultChecked);
    var val = p.checked !== undefined ? p.checked : st[0];
    return h("label", { className: "ec-check-row" },
      h("button", {
        type: "button", role: "checkbox", "aria-checked": String(!!val), className: "ec-check", id: p.id,
        onClick: function () { st[1](!val); p.onChange && p.onChange(!val); }
      }, h(Icon, { name: "check", size: 14, strokeWidth: 3 })),
      p.children ? h("span", null, p.children) : null);
  }

  function SegmentedControl(p) {
    var st = useState(p.value !== undefined ? p.value : (p.options[0] || {}).value);
    var v = p.onChange && p.value !== undefined ? p.value : st[0];
    return h("div", { className: cx("ec-seg", p.block && "ec-seg-block"), role: "group", "aria-label": p.label },
      p.options.map(function (o) {
        return h("button", {
          key: o.value, type: "button", "aria-pressed": String(o.value === v), "aria-label": o.label,
          onClick: function () { st[1](o.value); p.onChange && p.onChange(o.value); }
        }, o.icon ? h(Icon, { name: o.icon, size: 16 }) : null, o.text || null);
      }));
  }

  // ------------------------------------------------------------ inputs
  function TextField(p) {
    var o = rest(p, ["label", "icon", "error", "hint", "className", "id", "trailing"]);
    return h("div", { className: cx("ec-fieldwrap", p.className) },
      p.label ? h("label", { className: "ec-field-label", htmlFor: p.id }, p.label) : null,
      h("div", { className: cx("ec-field", p.error && "ec-field-invalid") },
        p.icon ? h(Icon, { name: p.icon, size: 18 }) : null,
        h("input", Object.assign({ id: p.id, "aria-invalid": p.error ? "true" : undefined }, o)),
        p.trailing || null),
      p.error ? h("div", { className: "ec-field-msg", role: "alert" }, p.error) : p.hint ? h("div", { className: "ec-field-hint" }, p.hint) : null);
  }

  // ------------------------------------------------------------ feedback
  function StatePill(p) {
    return h("span", { className: cx("ec-pill", p.tone && "ec-pill-" + p.tone, p.icon && "ec-pill-icon") },
      p.icon ? h(Icon, { name: p.icon, size: 13, strokeWidth: 2.4 }) : null, p.children);
  }

  function Spinner(p) { return h("span", { className: cx("ec-spinner", p.large && "ec-spinner-lg"), role: "status", "aria-label": p.label || "Working" }); }

  function ProgressBar(p) {
    return h("span", { className: cx("ec-progress", p.done && "ec-progress-done", p.value === undefined && "ec-progress-indet"), role: "progressbar", "aria-valuenow": p.value },
      h("i", { style: { width: (p.value || 0) + "%" } }));
  }

  function Snackbar(p) {
    return h("div", { className: cx("ec-snack", p.tone && "ec-snack-" + p.tone), role: "status" },
      p.icon ? h(Icon, { name: p.icon, size: 18 }) : null,
      h("span", { className: "ec-snack-text" }, p.children),
      p.action ? h("button", { type: "button", className: "ec-snack-action" }, p.action) : null);
  }

  function Banner(p) {
    var icon = p.icon || { warning: "alert", danger: "error", success: "check" }[p.tone] || "info";
    return h("div", { className: cx("ec-banner", p.tone && "ec-banner-" + p.tone) },
      h(Icon, { name: icon, size: 18 }),
      h("div", { className: "ec-banner-text" }, p.title ? h("strong", null, p.title) : null, p.children),
      p.action || null);
  }

  // ------------------------------------------------------------ structure
  function AppBar(p) {
    return h("header", { className: "ec-appbar" },
      p.back ? h(IconButton, { icon: "arrow-left", label: "Back" }) : null,
      h("div", { className: cx("ec-appbar-titles", !p.back && "ec-appbar-titles-flush") },
        h("h1", { className: "ec-appbar-title" }, p.title),
        p.sub ? h("span", { className: "ec-appbar-sub" }, p.sub) : null),
      (p.actions || []).map(function (a, i) { return h(IconButton, { key: i, icon: a.icon, label: a.label, pressed: a.pressed }); }));
  }

  var NAV = [["home", "Home"], ["clipboard", "Clipboard"], ["swap", "Transfers"], ["settings", "Settings"]];
  function BottomNav(p) {
    var st = useState(p.active || 0);
    var cur = p.onChange ? p.active || 0 : st[0];
    var badges = p.badges || {};
    return h("nav", { className: "ec-nav", "aria-label": "Sections" },
      NAV.map(function (it, i) {
        return h("button", {
          key: i, type: "button", className: "ec-nav-item", "aria-current": i === cur ? "page" : undefined,
          onClick: function () { st[1](i); p.onChange && p.onChange(i); }
        },
          h("span", { className: "ec-nav-glyph" }, h(Icon, { name: it[0], size: 22 }), badges[i] ? h("span", { className: "ec-nav-badge" }, badges[i]) : null),
          h("span", { className: "ec-nav-label" }, it[1]));
      }));
  }

  function SectionHeader(p) {
    return h("div", { className: "ec-section" },
      h("h2", { className: "ec-section-label" }, p.children),
      p.action ? h("button", { type: "button", className: "ec-section-action" }, p.action) : null);
  }

  function ListGroup(p) {
    return h("section", { className: "ec-group" },
      p.title ? h(SectionHeader, { action: p.action }, p.title) : null,
      h("div", { className: "ec-groupbox" }, p.children),
      p.foot ? h("p", { className: "ec-group-foot" }, p.foot) : null);
  }

  function ListRow(p) {
    var lead = p.icon ? h("span", { className: cx("ec-row-icon", p.tint && "ec-row-icon-" + p.tint) }, h(Icon, { name: p.icon, size: 20 }))
      : p.file ? h(FileIcon, { name: p.file, size: 36 })
      : p.avatar ? h("span", { className: "ec-avatar", style: { background: "var(--icon-" + (p.avatarColor || "purple") + ")" } }, p.avatar)
      : p.lead || null;
    var Tag = p.onClick || p.chevron ? "button" : "div";
    return h(Tag, { type: Tag === "button" ? "button" : undefined, className: cx("ec-row", p.sub && "ec-row-two", p.disabled && "ec-row-off"), onClick: p.onClick },
      lead,
      h("span", { className: "ec-row-text" },
        h("span", { className: "ec-row-title" }, p.title),
        p.sub ? h("span", { className: "ec-row-sub" }, p.sub) : null,
        p.below || null),
      p.trailing !== undefined ? h("span", { className: "ec-row-trail" }, p.trailing) : null,
      p.chevron ? h(Icon, { name: "chevron-right", size: 18, className: "ec-row-chev" }) : null);
  }

  function BottomSheet(p) {
    return h("div", { className: "ec-sheet", role: "dialog", "aria-label": p.title },
      h("span", { className: "ec-sheet-handle", "aria-hidden": "true" }),
      p.title ? h("div", { className: "ec-sheet-head" }, p.icon ? h(Icon, { name: p.icon, size: 20 }) : null, h("h2", { className: "ec-sheet-title" }, p.title)) : null,
      h("div", { className: "ec-sheet-body" }, p.children),
      p.footer ? h("div", { className: "ec-sheet-foot" }, p.footer) : null);
  }

  function Dialog(p) {
    return h("div", { className: cx("ec-dialog", p.tone === "danger" && "ec-dialog-danger"), role: "dialog", "aria-label": p.title },
      h("h2", { className: "ec-dialog-title" }, p.title),
      h("div", { className: "ec-dialog-body" }, p.children),
      h("div", { className: "ec-dialog-foot" }, p.actions));
  }

  // ------------------------------------------------------------ device
  /** A phone frame for whole screens: flat slate body, Android status bar, gesture bar. */
  function PhoneFrame(p) {
    var w = p.width || 360, hgt = p.height || 760;
    return h("div", { className: "ec-device", style: { width: w + 16 } },
      h("span", { className: "ec-device-key ec-device-vol", "aria-hidden": "true" }),
      h("span", { className: "ec-device-key ec-device-side", "aria-hidden": "true" }),
      h("div", { className: cx("ec", "ec-screen", p.dim && "ec-screen-dim"), style: { height: hgt } },
        h("div", { className: "ec-sysbar" },
          h("span", { className: "ec-sysbar-time" }, p.time || "10:42"),
          h("span", { className: "ec-sysbar-cam", "aria-hidden": "true" }),
          h("span", { className: "ec-sysbar-icons" },
            p.bluetooth !== false ? h(Icon, { name: "bluetooth", size: 13 }) : null,
            h(Icon, { name: "wifi", size: 14 }),
            h("span", { className: "ec-sysbar-batt" }, (p.battery || 72) + "%"))),
        h("div", { className: "ec-screen-body" }, p.children),
        h("div", { className: "ec-gesture", "aria-hidden": "true" }, h("i"))));
  }

  /** The laptop, drawn flat like the colour icons, with EchoFiles open on its screen. */
  function LaptopDevice(p) {
    var w = p.width || 220, s = p.state || "connected";
    var rows = [0, 1, 2, 3, 4, 5, 6];
    var scr = s === "away"
      ? [h("rect", { key: "off", x: 38, y: 14, width: 244, height: 150, fill: "#05060a" })]
      : s === "locked"
        ? [h("rect", { key: "l", x: 38, y: 14, width: 244, height: 150, style: { fill: "var(--bg-deep)" } }),
          h("text", { key: "t", x: 160, y: 84, textAnchor: "middle", fontSize: 26, fontWeight: 800, style: { fill: "var(--ink-strong)", fontFamily: "var(--font-mono)" } }, "10:42"),
          h("rect", { key: "f", x: 120, y: 102, width: 80, height: 14, rx: 2, style: { fill: "var(--bg)", stroke: "var(--accent)" }, strokeWidth: 1.5 }),
          h("text", { key: "u", x: 160, y: 136, textAnchor: "middle", fontSize: 8, style: { fill: "var(--ink-muted)", fontFamily: "var(--font-mono)" } }, "Locked from your phone")]
        : [h("rect", { key: "bg", x: 38, y: 14, width: 244, height: 150, style: { fill: "var(--bg-deep)" } }),
          // the EchoFiles window: 2px accent border, tabs, toolbar, sidebar, rows, status bar
          h("rect", { key: "win", x: 42, y: 18, width: 236, height: 142, style: { fill: "var(--bg)", stroke: "var(--accent)" }, strokeWidth: 1.5 }),
          h("rect", { key: "tabs", x: 43, y: 19, width: 234, height: 9, style: { fill: "var(--bg-deep)" } }),
          h("rect", { key: "tab", x: 43, y: 19, width: 46, height: 9, style: { fill: "var(--bg)" } }),
          h("rect", { key: "tabbar", x: 43, y: 19, width: 46, height: 1.5, style: { fill: "var(--accent)" } }),
          h("rect", { key: "tool", x: 60, y: 31, width: 120, height: 6, rx: 1, style: { fill: "var(--bg-deep)", stroke: "var(--line-strong)" }, strokeWidth: 0.6 }),
          h("rect", { key: "side", x: 43, y: 40, width: 52, height: 113, style: { fill: "var(--bg-sunken)" } }),
          h("g", { key: "marks" }, ["linux", "windows", "phone", "network"].map(function (wn, i) {
            return h("g", { key: wn },
              h("rect", { x: 48, y: 46 + i * 26, width: 3.5, height: 3.5, style: { fill: "var(--world-" + wn + ")" } }),
              h("rect", { x: 54, y: 46 + i * 26, width: 22, height: 3, rx: 1, style: { fill: "var(--ink-faint)" } }),
              h("rect", { x: 48, y: 54 + i * 26, width: 34, height: 3, rx: 1, style: { fill: i === 2 ? "var(--accent-ink)" : "var(--line-strong)" } }),
              h("rect", { x: 48, y: 61 + i * 26, width: 28, height: 3, rx: 1, style: { fill: "var(--line-strong)" } }));
          })),
          h("g", { key: "rows" }, rows.map(function (r) {
            return h("g", { key: r },
              r === 2 ? h("rect", { x: 97, y: 42 + r * 15, width: 179, height: 13, style: { fill: "var(--selection)" } }) : null,
              h("rect", { x: 101, y: 45 + r * 15, width: 9, height: 7, rx: 1, style: { fill: r < 3 ? "var(--icon-folder)" : r === 4 ? "var(--icon-red)" : "var(--icon-blue)" } }),
              h("rect", { x: 115, y: 47 + r * 15, width: 60 + (r * 23) % 50, height: 3, rx: 1, style: { fill: r === 2 ? "var(--ink-strong)" : "var(--ink-muted)" } }),
              h("rect", { x: 236, y: 47 + r * 15, width: 30, height: 3, rx: 1, style: { fill: "var(--ink-faint)" } }));
          })),
          h("rect", { key: "status", x: 43, y: 153, width: 234, height: 6, style: { fill: "var(--bg-deep)" } })];
    return h("div", { className: cx("ec-laptop", "ec-laptop-" + s), style: { width: w }, role: "img", "aria-label": LAPTOP + (s === "away" ? ", not nearby" : s === "locked" ? ", locked" : ", EchoFiles open") },
      h("svg", { viewBox: "0 0 320 196", width: "100%", style: { display: "block" } },
        h("rect", { x: 30, y: 6, width: 260, height: 166, rx: 10, style: { fill: "var(--icon-slate-deep)" } }),
        scr,
        h("circle", { cx: 160, cy: 10, r: 1.6, fill: "#000" }),
        h("path", { d: "M8 174h304l-6 14a6 6 0 0 1-5.5 4H19.5a6 6 0 0 1-5.5-4z", style: { fill: "var(--icon-slate)" } }),
        h("rect", { x: 136, y: 174, width: 48, height: 4, rx: 2, style: { fill: "var(--icon-slate-deep)" } })));
  }

  /** A small battery that fills to `level`, green when charging, warning at 20% and below. */
  function BatteryMeter(p) {
    var lv = Math.max(0, Math.min(100, p.level || 0));
    var tone = p.charging ? "var(--success)" : lv <= 10 ? "var(--danger)" : lv <= 20 ? "var(--warning)" : "currentColor";
    return h("span", { className: "ec-batt", role: "meter", "aria-label": "Battery " + lv + "%" + (p.charging ? ", charging" : ""), "aria-valuenow": lv },
      h("span", { className: "ec-batt-body" }, h("i", { style: { width: lv + "%", background: tone } })),
      p.charging ? h(Icon, { name: "bolt", size: 12, className: "ec-batt-bolt" }) : null,
      p.label === false ? null : h("span", null, lv + "%"));
  }

  /** How the phone reaches the laptop: Wi-Fi for everything, Bluetooth for calls and clipboard. */
  function LinkPills(p) {
    var wifi = p.wifi !== false, bt = p.bluetooth !== false;
    return h("span", { className: "ec-links" },
      h("span", { className: cx("ec-link", !wifi && "ec-link-off") }, h(Icon, { name: "wifi", size: 14 }), wifi ? "Wi-Fi · " + (p.network || "home") : "No Wi-Fi"),
      h("span", { className: cx("ec-link", !bt && "ec-link-off") }, h(Icon, { name: "bluetooth", size: 14 }), bt ? "Bluetooth" : "Bluetooth off"));
  }

  // ------------------------------------------------------------ connect pieces
  /** The laptop at the top of Home: drawing, name, connection, its battery. */
  function LaptopCard(p) {
    var s = p.state || "connected", away = s === "away";
    return h("div", { className: cx("ec-lapcard", away && "ec-lapcard-away") },
      h("div", { className: "ec-lapcard-stage" }, h(LaptopDevice, { state: away ? "away" : p.locked ? "locked" : "connected", width: p.deviceWidth || 236 })),
      h("div", { className: "ec-lapcard-facts" },
        h("div", { className: "ec-lapcard-top" },
          away ? h(StatePill, null, "Not nearby · 2 h ago") : h(StatePill, { tone: "success" }, "Connected"),
          away ? null : h(BatteryMeter, { level: p.battery === undefined ? 64 : p.battery, charging: p.charging !== false })),
        h("h2", { className: "ec-lapcard-name" }, h("span", { className: "ec-world", "aria-hidden": "true" }), LAPTOP),
        away ? h("p", { className: "ec-lapcard-hint" }, "Open the laptop on the same Wi-Fi. Calls and clipboard also work over Bluetooth when it's in range.")
          : h(LinkPills, { wifi: p.wifi, bluetooth: p.bluetooth, network: p.network })));
  }

  var ACTIONS = [
    { icon: "upload", label: "Send files", sub: "Photos, PDFs, anything" },
    { icon: "clipboard", label: "Send clipboard", sub: "What you copied last" },
    { icon: "external", label: "Open on laptop", sub: "A link, in its browser" },
    { icon: "lock", label: "Lock laptop", sub: "Locks the screen now" }];
  /** Two-by-two quick actions on Home. */
  function ActionGrid(p) {
    var items = p.items || ACTIONS;
    return h("div", { className: "ec-actions", role: "group", "aria-label": "Quick actions" },
      items.map(function (a, i) {
        return h("button", { key: i, type: "button", className: "ec-action", disabled: p.disabled },
          h("span", { className: "ec-action-icon" }, h(Icon, { name: a.icon, size: 20 })),
          h("span", { className: "ec-action-label" }, a.label),
          h("span", { className: "ec-action-sub" }, a.sub));
      }));
  }

  /** One thing that crossed the shared clipboard. */
  function ClipItem(p) {
    var to = p.dir !== "from";
    var text = p.sensitive ? "•••• •••• ••••" : p.text;
    return h("div", { className: cx("ec-clip", p.sensitive && "ec-clip-secret") },
      h("span", { className: cx("ec-clip-dir", to ? "ec-clip-to" : "ec-clip-from"), title: to ? "Phone to laptop" : "Laptop to phone" }, h(Icon, { name: to ? "arrow-up" : "arrow-down", size: 16 })),
      p.image ? h("img", { className: "ec-clip-img", src: photoSrc(p.image, "screenshot"), alt: "" }) : null,
      h("span", { className: "ec-clip-text" },
        h("span", { className: "ec-clip-body" }, p.image ? "Screenshot · 1080 × 2340" : text),
        h("span", { className: "ec-clip-meta" }, (to ? "To laptop" : "From laptop") + " · " + (p.when || "now") + (p.via === "bt" ? " · Bluetooth" : ""),
          p.sensitive ? h(StatePill, { icon: "eye-off" }, "Hidden") : null)),
      h(IconButton, { icon: "copy", label: "Copy again", size: 18 }));
  }

  /** How the phone's clipboard reaches the laptop: Automatic, paused, or Tap to send. */
  function ClipModeCard(p) {
    var m = p.mode || "auto";
    var pill = m === "auto" ? h(StatePill, { tone: "success" }, "Automatic")
      : m === "paused" ? h(StatePill, { tone: "warning" }, "Paused")
      : h(StatePill, null, "Tap to send");
    var body = m === "auto" ? "Copy anything on the phone and it's on the laptop. Set up 2 Oct."
      : m === "paused" ? "The phone restarted. Allow log access once to switch automatic sending back on."
      : "Send with Send to laptop in the text menu, the quick-settings tile, or the button below.";
    return h("div", { className: cx("ec-mode", "ec-mode-" + m) },
      h("div", { className: "ec-mode-head" }, h(Icon, { name: "clipboard", size: 20 }), h("strong", null, "Phone → laptop"), h("span", { className: "ec-grow" }), pill),
      h("p", { className: "ec-mode-body" }, body),
      m === "paused" ? h(Button, { variant: "primary", icon: "refresh", block: true }, "Resume automatic")
        : m === "manual" ? h(Button, { icon: "bolt", block: true }, "Make it automatic") : null,
      h("p", { className: "ec-mode-foot" }, h(Icon, { name: "check", size: 14 }), "Laptop → phone is always automatic."));
  }

  /** A file going to or coming from the laptop. */
  function TransferRow(p) {
    var going = p.progress !== undefined && p.progress < 100;
    var sub = p.failed ? h("span", { className: "ec-danger-ink" }, "Stopped · Wi-Fi dropped")
      : going ? (p.rate || "2.5 of 4.1 MB · 18 MB/s")
      : (p.size || "212 KB") + " · " + (p.dir === "from" ? "from laptop" : "to laptop") + " · " + (p.when || "2 min ago");
    return h(ListRow, {
      lead: h(FileIcon, { name: iconFor(p.name), size: 36 }), title: p.name, sub: sub,
      below: going ? h(ProgressBar, { value: p.progress }) : null,
      trailing: going ? h(IconButton, { icon: "close", label: "Stop", size: 18 }) : p.failed ? h(Button, { size: "sm" }, "Retry") : h(IconButton, { icon: "external", label: "Open", size: 18 })
    });
  }

  /** One Android permission: what it unlocks, its state, and how to grant it. */
  function PermissionRow(p) {
    var trail = p.granted ? h(StatePill, { tone: "success" }, "Allowed") : h(Button, { size: "sm", variant: p.optional ? undefined : "primary" }, p.action || "Allow");
    return h(ListRow, { icon: p.icon, tint: p.granted ? "ok" : null, title: p.title, sub: p.why, trailing: trail });
  }

  /** Steps down the screen: done ones collapse to a line, the current one opens. */
  function StepList(p) {
    var cur = p.current || 0;
    return h("ol", { className: "ec-steps" }, (p.steps || []).map(function (s, i) {
      var state = i < cur ? "done" : i === cur ? "now" : "todo";
      return h("li", { key: i, className: "ec-step ec-step-" + state },
        h("span", { className: "ec-step-n" }, state === "done" ? h(Icon, { name: "check", size: 13, strokeWidth: 3 }) : i + 1),
        h("div", { className: "ec-step-main" },
          h("span", { className: "ec-step-title" }, s.title),
          state === "now" && s.body ? h("div", { className: "ec-step-body" }, s.body) : null,
          state === "done" && s.done ? h("span", { className: "ec-step-done" }, s.done) : null));
    }));
  }

  /** Progress through onboarding: one bar per step, like the EchoFiles stepper. */
  function Stepper(p) {
    var cur = p.current || 0, n = p.count || 5;
    var bars = [];
    for (var i = 0; i < n; i++) bars.push(h("i", { key: i, className: i < cur ? "ec-bar-done" : i === cur ? "ec-bar-now" : undefined }));
    return h("div", { className: "ec-stepper", role: "progressbar", "aria-valuenow": cur + 1, "aria-valuemax": n, "aria-label": "Step " + (cur + 1) + " of " + n },
      h("span", { className: "ec-stepper-bars" }, bars),
      h("span", { className: "ec-stepper-text" }, "Step " + (cur + 1) + " of " + n + (p.label ? " · " + p.label : "")));
  }

  function qrCells(seed) {
    var n = 25, out = [], x = seed || 7;
    function finder(r, c) { return (r < 7 && c < 7) || (r < 7 && c >= n - 7) || (r >= n - 7 && c < 7); }
    for (var r = 0; r < n; r++) for (var c = 0; c < n; c++) {
      if (finder(r, c)) continue;
      x = (x * 1103515245 + 12345) & 0x7fffffff;
      if ((x >> 16) & 1) out.push([r, c]);
    }
    return out;
  }
  var QR = qrCells(11);
  /** The camera pointed at the code EchoFiles shows: the code, accent corner brackets, one status line. */
  function QrViewfinder(p) {
    var u = 6, o = 20;
    var finders = [[0, 0], [0, 18], [18, 0]].map(function (f, i) {
      return h("g", { key: i },
        h("rect", { x: o + f[1] * u, y: o + f[0] * u, width: 7 * u, height: 7 * u, fill: "#111" }),
        h("rect", { x: o + f[1] * u + u, y: o + f[0] * u + u, width: 5 * u, height: 5 * u, fill: "#fff" }),
        h("rect", { x: o + f[1] * u + 2 * u, y: o + f[0] * u + 2 * u, width: 3 * u, height: 3 * u, fill: "#111" }));
    });
    return h("div", { className: cx("ec-viewfinder", p.found && "ec-viewfinder-found") },
      h("div", { className: "ec-viewfinder-cam" },
        h("svg", { viewBox: "0 0 190 190", className: "ec-viewfinder-code", "aria-hidden": "true" },
          h("rect", { width: 190, height: 190, fill: "#fff" }),
          QR.map(function (c, i) { return h("rect", { key: i, x: o + c[1] * u, y: o + c[0] * u, width: u, height: u, fill: "#111" }); }),
          finders),
        h("span", { className: "ec-corner ec-corner-tl" }), h("span", { className: "ec-corner ec-corner-tr" }),
        h("span", { className: "ec-corner ec-corner-bl" }), h("span", { className: "ec-corner ec-corner-br" })),
      h("p", { className: "ec-viewfinder-text" }, p.found ? h("span", null, h(Icon, { name: "check", size: 16 }), "Found " + LAPTOP) : "Point at the code in EchoFiles → Connect phone"));
  }

  /** The four-pair code both screens show while pairing. */
  function PairCode(p) {
    var code = p.code || ["4F", "9A", "2C", "71"];
    return h("div", { className: "ec-paircode", "aria-label": "Pairing code " + code.join(" ") }, code.map(function (c, i) { return h("span", { key: i }, c); }));
  }

  /** A call running through the laptop's mic and speakers. */
  function CallBar(p) {
    return h("div", { className: "ec-callbar", role: "status" },
      h("span", { className: "ec-callbar-icon" }, h(Icon, { name: "call", size: 18 })),
      h("span", { className: "ec-callbar-text" },
        h("strong", null, (p.who || "Priya") + " · " + (p.time || "03:12")),
        h("span", null, "On laptop mic and speakers · Bluetooth")),
      h(Button, { size: "sm" }, "Use phone"));
  }

  // ------------------------------------------------------------ Android system surfaces (system font, not ours)
  function SystemNotification(p) {
    return h("div", { className: "ec-sysnote", style: { fontFamily: SYS_SANS } },
      h("div", { className: "ec-sysnote-head" },
        h("span", { className: "ec-sysnote-app" }, h(Icon, { name: "laptop", size: 11, strokeWidth: 2.4 })),
        h("span", null, "EchoConnect · now"), h("span", { className: "ec-grow" }), h(Icon, { name: "chevron-down", size: 14 })),
      h("strong", { className: "ec-sysnote-title" }, p.title || "Connected to " + LAPTOP),
      h("span", { className: "ec-sysnote-text" }, p.text || "Wi-Fi · Bluetooth · clipboard automatic"),
      h("div", { className: "ec-sysnote-actions" }, (p.actions || ["Send clipboard", "Disconnect"]).map(function (a, i) { return h("button", { key: i, type: "button" }, a); })));
  }

  function QuickTile(p) {
    return h("button", { type: "button", className: cx("ec-qstile", p.on && "ec-qstile-on"), style: { fontFamily: SYS_SANS } },
      h("span", { className: "ec-qstile-icon" }, h(Icon, { name: p.icon || "clipboard", size: 18 })),
      h("span", { className: "ec-qstile-text" }, h("strong", null, p.label || "Send clipboard"), h("span", null, p.sub || "EchoConnect")));
  }

  function SelectionMenu(p) {
    return h("div", { className: "ec-selmenu", role: "menu", style: { fontFamily: SYS_SANS } },
      ["Cut", "Copy", "Paste", "Send to laptop"].map(function (a, i) {
        return h("button", { key: i, type: "button", role: "menuitem", className: a === "Send to laptop" ? "ec-selmenu-ours" : undefined }, a === "Send to laptop" ? h(Icon, { name: "laptop", size: 14 }) : null, a);
      }),
      h("button", { type: "button", role: "menuitem", "aria-label": "More" }, h(Icon, { name: "more-vertical", size: 16 })));
  }

  function SystemDialog(p) {
    return h("div", { className: "ec-sysdialog", role: "alertdialog", style: { fontFamily: SYS_SANS } },
      h(Icon, { name: "terminal", size: 22 }),
      h("strong", null, p.title || "Allow EchoConnect to access all device logs?"),
      h("p", null, p.body || "Apps use logs to find and fix issues. Some logs may contain sensitive information, so only allow apps you trust."),
      h("div", { className: "ec-sysdialog-actions" }, (p.actions || ["Allow one-time access", "Don't allow"]).map(function (a, i) { return h("button", { key: i, type: "button" }, a); })));
  }

  // ------------------------------------------------------------ stand-in pictures (shared painter with EchoFiles)
  var SCENES = [
    ["#ff9a5a", "#c2477d", "#2b1846", "#ffd27a"], ["#7cc6ff", "#3b6fd8", "#13285c", "#fff6c9"], ["#ffcf8a", "#ff7b54", "#3a1f3d", "#fff1c1"],
    ["#a7e3c4", "#3d9a7a", "#0f3b36", "#f4ffd8"], ["#c9b6ff", "#6a4cd6", "#1d1647", "#ffe0f2"], ["#ffd6e0", "#e0637f", "#41203a", "#fff5e0"]];
  function photoSrc(i, kind) {
    var s = SCENES[i % SCENES.length], svg;
    if (kind === "screenshot") {
      svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 90 160"><rect width="90" height="160" fill="#101320"/><rect x="6" y="10" width="40" height="5" rx="2" fill="#e8ecff"/><rect x="6" y="22" width="78" height="34" rx="5" fill="' + s[1] + '"/>' +
        '<rect x="6" y="62" width="78" height="10" rx="3" fill="#232842"/><rect x="6" y="76" width="60" height="10" rx="3" fill="#232842"/><rect x="6" y="140" width="78" height="12" rx="6" fill="' + s[0] + '"/></svg>';
    } else {
      svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" preserveAspectRatio="xMidYMid slice"><rect width="100" height="100" fill="' + s[0] + '"/><rect y="62" width="100" height="38" fill="' + s[1] + '"/>' +
        '<circle cx="' + (30 + (i * 17) % 45) + '" cy="38" r="11" fill="' + s[3] + '"/><path d="M0 70 22 50l14 12 20-22 22 24 22-14v50H0z" fill="' + s[2] + '"/></svg>';
    }
    return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
  }

  // ------------------------------------------------------------ screens
  function Scroll(p) { return h("div", { className: "ec-scroll" }, p.children); }
  function App(p) { return h("div", { className: "ec-app" }, p.children); }

  var CLIPS = [
    { dir: "to", text: "https://maps.app.goo.gl/x7KdQ2", when: "1 min ago" },
    { dir: "from", text: "ssh aditya@build-box -p 2222", when: "6 min ago" },
    { dir: "to", sensitive: true, when: "22 min ago" },
    { dir: "to", image: 1, when: "40 min ago" },
    { dir: "from", text: "cargo run -p echofiles-phone --example fakephone", when: "1 h ago", via: "bt" }];

  /** Home: the laptop, what to do with it, the clipboard and the latest transfers. */
  function HomeScreen(p) {
    var s = p.state || "connected", away = s === "away";
    return h(App, null,
      h(AppBar, { title: "EchoConnect", actions: [{ icon: "qr", label: "Pair another laptop" }] }),
      h(Scroll, null,
        h(LaptopCard, { state: away ? "away" : "connected", locked: p.locked }),
        s === "call" ? h(CallBar, null) : null,
        h(ActionGrid, { disabled: away }),
        h(ListGroup, { title: "Clipboard", action: "See all" },
          h(ClipItem, CLIPS[0]), h(ClipItem, CLIPS[1])),
        h(ListGroup, { title: "Transfers", action: "See all" },
          h(TransferRow, { name: "IMG_20261002_1031.jpg", progress: 62 }),
          h(TransferRow, { name: "boarding-pass.pdf", dir: "from", when: "12 min ago" }))),
      h(BottomNav, { active: 0, badges: { 2: 1 } }));
  }

  /** Clipboard: how phone → laptop works, Send now, and the history both ways. */
  function ClipboardScreen(p) {
    return h(App, null,
      h(AppBar, { title: "Clipboard", actions: [{ icon: "trash", label: "Clear history" }] }),
      h(Scroll, null,
        h(ClipModeCard, { mode: p.mode || "auto" }),
        h(Button, { variant: p.mode === "manual" ? "primary" : undefined, icon: "send", block: true, size: "lg" }, "Send clipboard now"),
        h(SegmentedControl, { label: "Show", block: true, options: [{ value: "all", text: "All" }, { value: "to", text: "To laptop" }, { value: "from", text: "From laptop" }] }),
        h(ListGroup, { title: "Today", foot: "History stays on this phone for 24 hours. Passwords from password managers are hidden and never kept." },
          CLIPS.map(function (c, i) { return h(ClipItem, Object.assign({ key: i }, c)); }))),
      h(BottomNav, { active: 1 }));
  }

  /** Transfers: what's moving now, then what moved, newest first. */
  function TransfersScreen() {
    return h(App, null,
      h(AppBar, { title: "Transfers", actions: [{ icon: "upload", label: "Send files" }] }),
      h(Scroll, null,
        h(ListGroup, { title: "Now" },
          h(TransferRow, { name: "VID_20261002_0912.mp4", progress: 38, rate: "31 of 84 MB · 22 MB/s · 3 s left" }),
          h(TransferRow, { name: "IMG_20261002_1031.jpg", progress: 62 })),
        h(ListGroup, { title: "Today" },
          h(TransferRow, { name: "boarding-pass.pdf", dir: "from", when: "12 min ago" }),
          h(TransferRow, { name: "Q3-final.xlsx", size: "1.4 MB", dir: "from", when: "09:40" }),
          h(TransferRow, { name: "voice-note.opus", size: "88 KB", failed: true })),
        h(ListGroup, { title: "Yesterday", foot: "Files from the laptop save to Download/EchoConnect. Files you send land in ~/Downloads/Phone on the laptop." },
          h(TransferRow, { name: "scan-lease.pdf", size: "2.1 MB", when: "Yesterday 18:22" }),
          h(TransferRow, { name: "echoconnect-debug.apk", size: "14 MB", dir: "from", when: "Yesterday 11:05" }))),
      h(BottomNav, { active: 2 }));
  }

  /** Settings: the laptop, what's shared, receiving, look, and about. */
  function SettingsScreen() {
    return h(App, null,
      h(AppBar, { title: "Settings" }),
      h(Scroll, null,
        h(ListGroup, { title: "Laptop" },
          h(ListRow, { lead: h(LaptopDevice, { width: 52 }), title: LAPTOP, sub: "Paired 2 Oct · EchoFiles 0.4" }),
          h(ListRow, { icon: "wifi", title: "Wi-Fi", sub: "Files, photos, texts, notifications", trailing: h(StatePill, { tone: "success" }, "home") }),
          h(ListRow, { icon: "bluetooth", title: "Bluetooth", sub: "Calls and clipboard, even off Wi-Fi", trailing: h(StatePill, { tone: "success" }, "On") }),
          h(ListRow, { icon: "unlink", title: "Forget this laptop", sub: "Pair again with its code to undo", chevron: true })),
        h(ListGroup, { title: "Shared with the laptop" },
          h(ListRow, { icon: "bell", title: "Notifications", sub: "Shown on the laptop; reply from there", trailing: h(Switch, { defaultChecked: true, label: "Notifications" }) }),
          h(ListRow, { icon: "message", title: "Messages", sub: "Read and send texts from the laptop", trailing: h(Switch, { defaultChecked: true, label: "Messages" }) }),
          h(ListRow, { icon: "call", title: "Calls on the laptop", sub: "Answer on laptop connects Bluetooth audio", trailing: h(Switch, { defaultChecked: true, label: "Calls on the laptop" }) }),
          h(ListRow, { icon: "image", title: "Files and photos", sub: "Browse the phone from EchoFiles", trailing: h(Switch, { defaultChecked: true, label: "Files and photos" }) }),
          h(ListRow, { icon: "clipboard", title: "Clipboard", sub: "Automatic · set up 2 Oct", chevron: true })),
        h(ListGroup, { title: "Receiving" },
          h(ListRow, { icon: "download", title: "Save files to", sub: "Download/EchoConnect", chevron: true }),
          h(ListRow, { icon: "check", title: "Accept files automatically", sub: "Off asks before each file", trailing: h(Switch, { defaultChecked: true, label: "Accept files automatically" }) })),
        h(ListGroup, { title: "Look" },
          h(ListRow, { icon: "sliders", title: "Theme", sub: "Match laptop uses its Omarchy theme", below: h(SegmentedControl, { label: "Theme", block: true, value: "laptop", options: [{ value: "laptop", text: "Match laptop" }, { value: "system", text: "Phone" }] }) })),
        h(ListGroup, { title: "This phone" },
          h(ListRow, { icon: "shield", title: "Permissions", sub: "8 of 9 allowed", chevron: true }),
          h(ListRow, { icon: "battery", title: "Stay connected", sub: "Never sleeping on Samsung", trailing: h(StatePill, { tone: "success" }, "Set") })),
        h(ListGroup, { title: "About", foot: "EchoConnect and EchoFiles are open source (GPL-3.0). Read every line at github.com/adityavardhansharma/EchoFiles_Linux." },
          h(ListRow, { icon: "info", title: "EchoConnect 0.1.0", sub: "Android 14 · One UI 6.1" }),
          h(ListRow, { icon: "code", title: "Source code", sub: "github.com/adityavardhansharma/EchoFiles_Linux", trailing: h(Icon, { name: "external", size: 18 }) }))),
      h(BottomNav, { active: 3 }));
  }

  var PERMS = [
    { icon: "bell", title: "Notification access", why: "Shows your notifications on the laptop", granted: true },
    { icon: "message", title: "SMS", why: "Read and send texts from the laptop", granted: true },
    { icon: "users", title: "Contacts", why: "Names instead of numbers in texts and calls", granted: true },
    { icon: "call", title: "Phone", why: "Caller name on the laptop; answer from there", granted: true },
    { icon: "bluetooth", title: "Nearby devices", why: "Bluetooth for calls and clipboard", granted: true },
    { icon: "image", title: "All files access", why: "Browse the phone from EchoFiles", granted: true },
    { icon: "clipboard", title: "Display over other apps", why: "Lets automatic clipboard read what you copy", granted: false, action: "Open setting" },
    { icon: "battery", title: "Battery: Unrestricted", why: "Stays connected with the screen off", granted: true },
    { icon: "bell", title: "Notifications", why: "The small always-on Connected notification", granted: true }];
  /** Permissions: each one, what it unlocks, and its state; nothing hidden. */
  function PermissionsScreen() {
    return h(App, null,
      h(AppBar, { title: "Permissions", back: true }),
      h(Scroll, null,
        h("p", { className: "ec-lede" }, "Each permission unlocks one feature. Turn a feature off in Settings and EchoConnect stops using its permission."),
        h(ListGroup, { title: "Allowed · 8 of 9" }, PERMS.map(function (pm, i) { return h(PermissionRow, Object.assign({ key: i }, pm)); })),
        h(Banner, { title: "On Samsung", icon: "battery" }, " Settings → Battery → Background usage limits → Never sleeping apps → add EchoConnect. Otherwise One UI may disconnect it overnight.")));
  }

  var SETUP_STEPS = [
    { title: "Turn on Developer options", done: "On",
      body: [h("p", { key: "a" }, "Settings → About phone → Software information → tap ", h("strong", null, "Build number"), " 7 times. Enter your PIN when asked."),
        h(Button, { key: "b", icon: "external", block: true }, "Open Software information")] },
    { title: "Turn on Wireless debugging", done: "On",
      body: [h("p", { key: "a" }, "Settings → Developer options → ", h("strong", null, "Wireless debugging"), " → Allow on this Wi-Fi. Then tap ", h("strong", null, "Pair device with QR code"), "."),
        h(Button, { key: "b", icon: "external", block: true }, "Open Developer options")] },
    { title: "Scan the code on the laptop", done: "Permission granted",
      body: [h("p", { key: "a" }, "On the laptop: EchoFiles → Settings → Phone → ", h("strong", null, "Make clipboard automatic"), ". Scan its code from the Wireless debugging screen."),
        h("div", { key: "b", className: "ec-wait" }, h(Spinner, null), "Waiting for " + LAPTOP + "…")] },
    { title: "Allow Display over other apps", done: "Allowed",
      body: [h("p", { key: "a" }, "A normal setting. It lets EchoConnect open an invisible window for a moment to read what you copied."),
        h(Button, { key: "b", variant: "primary", icon: "external", block: true }, "Open the setting")] },
    { title: "Turn Developer options off", done: "Off · banking apps work normally",
      body: [h("p", { key: "a" }, "The permission stays. Banking and payment apps only check whether Developer options are on now."),
        h(Button, { key: "b", variant: "primary", icon: "external", block: true }, "Open Developer options"),
        h("div", { key: "c", className: "ec-wait" }, h(StatePill, { tone: "warning" }, "Still on"), "EchoConnect checks again on its own.")] }];
  /** The one-time Automatic clipboard setup, run together with EchoFiles on the laptop. */
  function ClipboardSetupScreen(p) {
    var st = useState(p.step === undefined ? 2 : p.step);
    return h(App, null,
      h(AppBar, { title: "Automatic clipboard", back: true }),
      h(Scroll, null,
        h("p", { className: "ec-lede" }, "About 2 minutes, once. Afterwards you copy on the phone and paste on the laptop — no taps."),
        h(StepList, { steps: SETUP_STEPS, current: st[0] }),
        h("div", { className: "ec-setup-nav" },
          h(Button, { variant: "ghost", onClick: function () { st[1](Math.max(0, st[0] - 1)); } }, "Back"),
          h("span", { className: "ec-grow" }),
          h(Button, { onClick: function () { st[1](Math.min(5, st[0] + 1)); } }, st[0] >= 4 ? "Finish" : "Next step")),
        h(Banner, { title: "What EchoConnect reads", icon: "shield" },
          " Only the one line Android writes when a clipboard read is blocked. It never stores or sends anything else from the log. EchoConnect is open source — check the code at github.com/adityavardhansharma/EchoFiles_Linux."),
        h(Banner, { title: "After a restart", icon: "refresh" }, " Android asks once: Allow EchoConnect to access all device logs? Tap Allow one-time access and it works until the next restart."),
        h(Button, { variant: "ghost", block: true }, "Use Tap to send instead")));
  }

  var PAIR_STEPS = ["Welcome", "Scan", "Check code", "Permissions", "Clipboard"];
  /** First run: welcome, scan the laptop's code, check the code, permissions, clipboard choice. */
  function PairingScreen(p) {
    var st = useState(p.step || 0);
    var step = st[0], next = function () { st[1](Math.min(5, step + 1)); };
    var body, foot;
    if (step === 0) {
      body = [h("div", { key: "a", className: "ec-hero" }, h(LaptopDevice, { width: 260 })),
        h("h1", { key: "b", className: "ec-display" }, "Your phone, on your laptop"),
        h("p", { key: "c", className: "ec-lede" }, "Files, photos, clipboard, texts, notifications and calls between this phone and EchoFiles — over your Wi-Fi, with Bluetooth for calls."),
        h("ul", { key: "d", className: "ec-ticks" }, ["Nothing leaves your network", "Encrypted, paired once", "Open source"].map(function (t, i) { return h("li", { key: i }, h(Icon, { name: "check", size: 16 }), t); }))];
      foot = h(Button, { variant: "primary", size: "lg", block: true, icon: "scan", onClick: next }, "Scan the laptop's code");
    } else if (step === 1) {
      body = [h("p", { key: "a", className: "ec-lede" }, "On the laptop, open EchoFiles and click ", h("strong", null, "Connect phone"), " in the sidebar. A code appears."),
        h(QrViewfinder, { key: "b", found: p.found })];
      foot = h(Button, { variant: "ghost", block: true }, "Type the laptop's address instead");
    } else if (step === 2) {
      body = [h("p", { key: "a", className: "ec-lede" }, "Check the laptop shows the same code. Pairing is approved on both at once."),
        h(PairCode, { key: "b" }),
        h("div", { key: "c", className: "ec-wait" }, h(Spinner, null), "Pairing with " + LAPTOP + "…")];
      foot = [h(Button, { key: "a", variant: "primary", size: "lg", block: true, onClick: next }, "Codes match"), h(Button, { key: "b", variant: "ghost", block: true }, "They don't match")];
    } else if (step === 3) {
      body = [h("p", { key: "a", className: "ec-lede" }, "Allow what you want on the laptop. Skip any — you can turn it on later."),
        h(ListGroup, { key: "b" }, PERMS.slice(0, 6).map(function (pm, i) { return h(PermissionRow, Object.assign({ key: i }, pm, { granted: i < 3 })); }))];
      foot = h(Button, { variant: "primary", size: "lg", block: true, onClick: next }, "Continue");
    } else if (step === 4) {
      body = [h("p", { key: "a", className: "ec-lede" }, "How should what you copy on the phone reach the laptop?"),
        h("button", { key: "b", type: "button", className: "ec-choice", "aria-pressed": "true" },
          h("span", { className: "ec-choice-head" }, h(Icon, { name: "bolt", size: 18 }), h("strong", null, "Automatic"), h(StatePill, { tone: "accent" }, "Recommended")),
          h("span", null, "Copy on the phone, paste on the laptop. One-time setup with the laptop, about 2 minutes; Developer options go back off after.")),
        h("button", { key: "c", type: "button", className: "ec-choice", "aria-pressed": "false" },
          h("span", { className: "ec-choice-head" }, h(Icon, { name: "send", size: 18 }), h("strong", null, "Tap to send")),
          h("span", null, "Use Send to laptop in the text menu, the quick-settings tile, or Share. No setup.")),
        h("p", { key: "d", className: "ec-note" }, h(Icon, { name: "check", size: 14 }), "Laptop → phone is automatic either way.")];
      foot = h(Button, { variant: "primary", size: "lg", block: true, onClick: next }, "Set up Automatic");
    } else {
      body = [h("div", { key: "a", className: "ec-hero" }, h(LaptopDevice, { width: 260 })),
        h("h1", { key: "b", className: "ec-display" }, "Connected"),
        h("p", { key: "c", className: "ec-lede" }, LAPTOP + " can now see this phone. EchoConnect keeps a small notification while it's connected.")];
      foot = h(Button, { variant: "primary", size: "lg", block: true, onClick: function () { st[1](0); } }, "Done");
    }
    return h(App, null,
      h("div", { className: "ec-flowhead" }, step > 0 && step < 5 ? h(IconButton, { icon: "arrow-left", label: "Back", onClick: function () { st[1](step - 1); } }) : h("span", { className: "ec-flowhead-pad" }),
        step < 5 ? h(Stepper, { current: step, count: 5, label: PAIR_STEPS[step] }) : h(Stepper, { current: 5, count: 5, label: "Done" })),
      h(Scroll, null, body),
      h("div", { className: "ec-flowfoot" }, foot));
  }

  /** Full screen while the laptop rings the phone. */
  function RingScreen() {
    return h("div", { className: "ec-ring" },
      h("div", { className: "ec-ring-mark", "aria-hidden": "true" }, h("i"), h("i"), h("span", null, h(Icon, { name: "phone-ring", size: 40 }))),
      h("h1", { className: "ec-display" }, "Ringing"),
      h("p", { className: "ec-lede" }, LAPTOP + " is looking for this phone. Full volume, even on silent; the flashlight blinks too."),
      h("span", { className: "ec-grow" }),
      h(Button, { variant: "primary", size: "lg", block: true, icon: "close" }, "Stop ringing"),
      h("p", { className: "ec-ring-foot" }, "Any volume key stops it too."));
  }

  /** Share → EchoConnect from any app: what goes, where it lands, Send. */
  function ShareSheetScreen() {
    return h(App, null,
      h("div", { className: "ec-gallery", "aria-hidden": "true" }, [0, 1, 2, 3, 4, 5, 0, 2, 4, 1, 3, 5, 2, 0, 1].map(function (k, i) {
        return h("span", { key: i, className: [1, 4, 6].indexOf(i) >= 0 ? "ec-gallery-sel" : undefined }, h("img", { src: photoSrc(k), alt: "" }));
      })),
      h("div", { className: "ec-scrim" }),
      h(BottomSheet, { title: "Send to " + LAPTOP, icon: "laptop",
        footer: [h(Button, { key: "c", variant: "ghost" }, "Cancel"), h(Button, { key: "s", variant: "primary", icon: "send" }, "Send 3 photos")] },
        h("div", { className: "ec-sharethumbs" }, [1, 4, 0].map(function (k, i) { return h("img", { key: i, src: photoSrc(k), alt: "" }); })),
        h(ListRow, { icon: "folder", title: "Lands in ~/Downloads/Phone", sub: "11.8 MB · over Wi-Fi, about 1 second" }),
        h(ListRow, { icon: "check", title: "Open on the laptop when done", trailing: h(Switch, { label: "Open on the laptop when done" }) })));
  }

  /** The Android-side pieces EchoConnect adds: notification, tile, text menu, log prompt. */
  function SystemSurfaces() {
    return h("div", { className: "ec-surfaces" },
      h("figure", null, h(PhoneFrame, { height: 620 },
        h("div", { className: "ec-shade" },
          h("div", { className: "ec-shade-tiles" }, h(QuickTile, { on: true }), h(QuickTile, { icon: "wifi", label: "Wi-Fi", sub: "home", on: true }), h(QuickTile, { icon: "bluetooth", label: "Bluetooth", sub: LAPTOP, on: true }), h(QuickTile, { icon: "moon", label: "Do not disturb", sub: "Synced with laptop" })),
          h(SystemNotification, null),
          h(SystemNotification, { title: "Priya · on laptop", text: "Call on laptop mic and speakers · 03:12", actions: ["Use phone", "Hang up"] }))),
        h("figcaption", null, "Notification shade: the always-on notification, the Send clipboard tile, a call on the laptop.")),
      h("figure", null, h(PhoneFrame, { height: 620 },
        h(App, null,
          h("div", { className: "ec-chat" },
            h("span", { className: "ec-chat-bubble" }, "Wi-Fi password for the office is "),
            h("span", { className: "ec-chat-bubble" }, h("mark", null, "blue-otter-42"), " — don't share it"),
            h(SelectionMenu, null)))),
        h("figcaption", null, "Text selection: Send to laptop sits next to Copy — the no-setup way.")),
      h("figure", null, h(PhoneFrame, { height: 620, dim: true },
        h("div", { className: "ec-dialogstage" }, h(SystemDialog, null))),
        h("figcaption", null, "After a restart: Android's log prompt. Allow one-time access resumes Automatic.")));
  }

  /** Every screen side by side: the app at a glance. */
  function AppMap() {
    var cells = [["Home", h(HomeScreen, null)], ["Home · call on laptop", h(HomeScreen, { state: "call" })], ["Clipboard", h(ClipboardScreen, null)], ["Transfers", h(TransfersScreen, null)],
      ["Settings", h(SettingsScreen, null)], ["Pairing · scan", h(PairingScreen, { step: 1, found: true })], ["Automatic clipboard", h(ClipboardSetupScreen, null)], ["Ringing", h(RingScreen, null)]];
    return h("div", { className: "ec-map" }, cells.map(function (c, i) {
      return h("figure", { key: i }, h(PhoneFrame, { height: 720 }, c[1]), h("figcaption", null, c[0]));
    }));
  }

  window.EchoConnect = {
    Icon: Icon, FileIcon: FileIcon, iconFor: iconFor, Button: Button, IconButton: IconButton, Switch: Switch, Checkbox: Checkbox,
    SegmentedControl: SegmentedControl, TextField: TextField, StatePill: StatePill, Spinner: Spinner, ProgressBar: ProgressBar,
    Snackbar: Snackbar, Banner: Banner, AppBar: AppBar, BottomNav: BottomNav, SectionHeader: SectionHeader, ListGroup: ListGroup,
    ListRow: ListRow, BottomSheet: BottomSheet, Dialog: Dialog, PhoneFrame: PhoneFrame, LaptopDevice: LaptopDevice,
    BatteryMeter: BatteryMeter, LinkPills: LinkPills, LaptopCard: LaptopCard, ActionGrid: ActionGrid, ClipItem: ClipItem,
    ClipModeCard: ClipModeCard, TransferRow: TransferRow, PermissionRow: PermissionRow, StepList: StepList, Stepper: Stepper,
    QrViewfinder: QrViewfinder, PairCode: PairCode, CallBar: CallBar, SystemNotification: SystemNotification, QuickTile: QuickTile,
    SelectionMenu: SelectionMenu, SystemDialog: SystemDialog, HomeScreen: HomeScreen, ClipboardScreen: ClipboardScreen,
    TransfersScreen: TransfersScreen, SettingsScreen: SettingsScreen, PermissionsScreen: PermissionsScreen,
    ClipboardSetupScreen: ClipboardSetupScreen, PairingScreen: PairingScreen, RingScreen: RingScreen,
    ShareSheetScreen: ShareSheetScreen, SystemSurfaces: SystemSurfaces, AppMap: AppMap, photoSrc: photoSrc
  };
})();
