(function () {
  var React = window.React;
  var h = React.createElement;
  var useState = React.useState, useEffect = React.useEffect, useRef = React.useRef;

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

  // ------------------------------------------------------------ icons
  function Icon(p) {
    var size = p.size || 16;
    return h("svg", {
      className: cx("ef-icon", p.className), width: size, height: size, viewBox: "0 0 24 24",
      fill: "none", stroke: "currentColor", strokeWidth: p.strokeWidth || 2, strokeLinecap: "round",
      strokeLinejoin: "round", "aria-hidden": p.label ? undefined : "true", role: p.label ? "img" : undefined,
      "aria-label": p.label, dangerouslySetInnerHTML: { __html: ICONS.glyphs[p.name] || "" }
    });
  }

  var EXT = {
    jpg: "file-image", jpeg: "file-image", png: "file-image", webp: "file-image", heic: "file-image", gif: "file-image", avif: "file-image",
    mp4: "file-video", mkv: "file-video", mov: "file-video", webm: "file-video",
    mp3: "file-audio", flac: "file-audio", ogg: "file-audio", wav: "file-audio", m4a: "file-audio",
    pdf: "file-pdf", doc: "file-doc", docx: "file-doc", odt: "file-doc", md: "file-text", txt: "file-lines",
    xls: "file-sheet", xlsx: "file-sheet", csv: "file-sheet", ods: "file-sheet", ppt: "file-slides", pptx: "file-slides", odp: "file-slides",
    rs: "file-code", js: "file-code", ts: "file-code", py: "file-code", c: "file-code", html: "file-code", json: "file-code",
    toml: "file-config", ini: "file-config", conf: "file-config", yaml: "file-config", yml: "file-config",
    sh: "file-script", ps1: "file-script", bat: "file-script", exe: "file-script",
    zip: "file-archive", "7z": "file-archive", tar: "file-archive-alt", gz: "file-archive-alt", zst: "file-archive-alt", rar: "file-package",
    ttf: "file-font", otf: "file-font", woff2: "file-font", svg: "file-vector", lnk: "file", iso: "drive"
  };
  function iconFor(name, kind) {
    if (kind === "folder") return "folder";
    var m = /\.([^.]+)$/.exec(name || "");
    return (m && EXT[m[1].toLowerCase()]) || "file";
  }

  function FileIcon(p) {
    var size = p.size || 18;
    var name = p.name || "file";
    var badge = null;
    if (p.badge === "link") badge = h("span", { className: "ef-badge", title: "Link" }, h(Icon, { name: "link", size: 9, strokeWidth: 3 }));
    if (p.badge === "cloud") badge = h("span", { className: "ef-badge ef-badge-cloud", title: "Cloud-only placeholder" }, h(Icon, { name: "cloud", size: 9, strokeWidth: 3 }));
    if (p.badge === "lock") badge = h("span", { className: "ef-badge", title: "Encrypted (EFS)" }, h(Icon, { name: "lock", size: 9, strokeWidth: 3 }));
    if (p.badge === "broken") badge = h("span", { className: "ef-badge ef-badge-broken", title: "Broken link" }, h(Icon, { name: "close", size: 9, strokeWidth: 3.5 }));
    if (p.badge === "ads") badge = h("span", { className: "ef-badge ef-badge-ads", title: "Has alternate data streams" }, "+ADS");
    return h("span", { className: cx("ef-ficon", p.className), style: { width: size, height: size }, "aria-hidden": "true" },
      h("svg", { viewBox: "0 0 48 48", dangerouslySetInnerHTML: { __html: ICONS.color[name] || ICONS.color.file } }),
      size >= 18 ? badge : null);
  }

  // ------------------------------------------------------------ actions
  function Kbd(p) {
    var keys = p.keys || [];
    return h("span", { className: "ef-kbd", "aria-label": keys.join("+") }, keys.map(function (k, i) { return h("kbd", { key: i }, k); }));
  }

  function Button(p) {
    var o = rest(p, ["variant", "size", "icon", "kbd", "className", "children"]);
    o.className = cx("ef-btn", p.variant && "ef-btn-" + p.variant, p.size === "sm" && "ef-btn-sm", p.className);
    o.type = o.type || "button";
    return h("button", o, p.icon ? h(Icon, { name: p.icon, size: 14 }) : null, p.children, p.kbd ? h(Kbd, { keys: p.kbd }) : null);
  }

  function IconButton(p) {
    var o = rest(p, ["icon", "label", "pressed", "className"]);
    o.className = cx("ef-ibtn", p.className);
    o.type = "button";
    o["aria-label"] = p.label;
    o.title = p.label;
    if (p.pressed !== undefined) o["aria-pressed"] = String(!!p.pressed);
    return h("button", o, h(Icon, { name: p.icon, size: 16 }));
  }

  function SegmentedControl(p) {
    var st = useState(p.value !== undefined ? p.value : (p.options[0] || {}).value);
    var v = p.value !== undefined ? p.value : st[0];
    return h("div", { className: "ef-seg", role: "group", "aria-label": p.label },
      p.options.map(function (o) {
        return h("button", {
          key: o.value, type: "button", "aria-pressed": String(o.value === v), title: o.title || o.label,
          "aria-label": o.label || o.title,
          onClick: function () { st[1](o.value); p.onChange && p.onChange(o.value); }
        }, o.icon ? h(Icon, { name: o.icon, size: 14 }) : null, o.text || null);
      }));
  }

  function Switch(p) {
    var st = useState(!!p.defaultChecked);
    var on = p.checked !== undefined ? p.checked : st[0];
    var sw = h("button", {
      type: "button", role: "switch", "aria-checked": String(on), className: "ef-switch", id: p.id,
      "aria-label": p.children ? undefined : p.label,
      onClick: function () { st[1](!on); p.onChange && p.onChange(!on); }
    });
    if (!p.children) return sw;
    return h("label", { className: "ef-toggle-row" }, h("span", null, p.children), sw);
  }

  function Checkbox(p) {
    var st = useState(!!p.defaultChecked);
    var val = p.checked !== undefined ? p.checked : st[0];
    var aria = val === "mixed" ? "mixed" : String(!!val);
    return h("label", { className: "ef-label" },
      h("button", {
        type: "button", role: "checkbox", "aria-checked": aria, className: "ef-check", id: p.id,
        onClick: function () { var n = val === true ? false : true; st[1](n); p.onChange && p.onChange(n); }
      }, h(Icon, { name: aria === "mixed" ? "minus" : "check", size: 12, strokeWidth: 3 })),
      p.children ? h("span", null, p.children) : null);
  }

  // ------------------------------------------------------------ inputs
  function TextField(p) {
    var o = rest(p, ["label", "icon", "error", "className", "trailing", "id"]);
    return h("div", { className: p.className },
      p.label ? h("label", { className: "ef-field-label", htmlFor: p.id }, p.label) : null,
      h("div", { className: cx("ef-field", p.error && "ef-field-invalid") },
        p.icon ? h(Icon, { name: p.icon, size: 14 }) : null,
        h("input", Object.assign({ id: p.id, "aria-invalid": p.error ? "true" : undefined }, o)),
        p.trailing || null),
      p.error ? h("div", { className: "ef-field-msg", role: "alert" }, p.error) : null);
  }

  function SearchField(p) {
    return h("div", { className: cx("ef-field ef-search", p.className), role: "search" },
      h(Icon, { name: "search", size: 14 }),
      h("input", { id: p.id, type: "search", placeholder: p.placeholder || "Search", defaultValue: p.defaultValue, "aria-label": "Search" }),
      p.scope ? h("span", { className: "ef-chip" }, p.scope) : h(Kbd, { keys: ["Ctrl", "F"] }));
  }

  function PathBar(p) {
    var st = useState(!!p.editing);
    var editing = st[0];
    var segs = p.segments || [];
    if (editing) {
      return h("div", { className: "ef-field ef-path-edit" },
        h(Icon, { name: "folder", size: 14 }),
        h("input", { id: p.id, autoFocus: true, defaultValue: p.path || "/" + segs.map(function (s) { return s.label; }).join("/"), "aria-label": "Location", onBlur: function () { st[1](false); } }),
        h(Kbd, { keys: ["Enter"] }));
    }
    var kids = [];
    segs.forEach(function (s, i) {
      if (i > 0) kids.push(h(Icon, { key: "sep" + i, name: "chevron-right", size: 12, className: "ef-path-sep" }));
      kids.push(h("button", {
        key: i, type: "button", className: cx("ef-path-seg", i === segs.length - 1 && p.animateLast && "ef-path-new"),
        "aria-current": i === segs.length - 1 ? "location" : undefined
      }, s.icon ? h(Icon, { name: s.icon, size: 14 }) : null, s.label));
    });
    return h("nav", { className: "ef-path", "aria-label": "Location", onDoubleClick: function () { st[1](true); }, title: "Double-click or Ctrl+L to type a path" }, kids);
  }

  // ------------------------------------------------------------ navigation
  function TabStrip(p) {
    var st = useState(p.active || 0);
    return h("div", { className: "ef-tabs", role: "tablist" },
      (p.tabs || []).map(function (t, i) {
        return h("button", { key: i, type: "button", role: "tab", className: "ef-tab", "aria-selected": String(i === st[0]), onClick: function () { st[1](i); } },
          h(Icon, { name: t.icon || "folder", size: 14 }),
          h("span", { className: "ef-tab-name" }, t.label),
          h("span", { className: "ef-tab-close", "aria-label": "Close tab" }, h(Icon, { name: "close", size: 12 })));
      }),
      h("button", { type: "button", className: "ef-tab-add", "aria-label": "New tab", title: "New tab (Ctrl+T)" }, h(Icon, { name: "plus", size: 14 })));
  }

  function Toolbar(p) {
    return h("div", { className: "ef-toolbar", role: "toolbar", "aria-label": "Navigation" },
      h(IconButton, { icon: "arrow-left", label: "Back (Alt+Left)" }),
      h(IconButton, { icon: "arrow-right", label: "Forward (Alt+Right)", disabled: true }),
      h(IconButton, { icon: "arrow-up", label: "Parent folder (Alt+Up)" }),
      h("span", { className: "ef-toolbar-sep" }),
      h(PathBar, { segments: p.segments || [], animateLast: true }),
      h("span", { className: "ef-toolbar-sep" }),
      h(SearchField, { scope: p.scope }),
      h(SegmentedControl, { label: "View", value: p.view || "list", options: [
        { value: "list", icon: "list", label: "List view (Ctrl+1)" },
        { value: "grid", icon: "grid", label: "Grid view (Ctrl+2)" },
        { value: "columns", icon: "columns", label: "Dual pane (F3)" }] }),
      h(IconButton, { icon: "sidebar", label: "Preview pane (Space)", pressed: !!p.preview }),
      h(IconButton, { icon: "more-vertical", label: "More" }));
  }

  function Sidebar(p) {
    return h("nav", { className: "ef-sidebar", "aria-label": "Places" }, p.children);
  }
  function SidebarSection(p) {
    return h("section", null,
      h("div", { className: "ef-sec-head" },
        p.world ? h("span", { className: "ef-sec-mark", style: { background: "var(--world-" + p.world + ")" } }) : null,
        p.title, p.count !== undefined ? h("span", { className: "ef-sec-count" }, p.count) : null),
      h("div", { className: "ef-sec-list" }, p.children));
  }
  function SidebarItem(p) {
    return h("button", { type: "button", className: cx("ef-side-item", p.dropTarget && "ef-side-drop"), "aria-current": p.active ? "true" : undefined },
      h(Icon, { name: p.icon || "folder", size: 16 }),
      h("span", { className: "ef-side-name" }, p.label),
      p.trail ? h("span", { className: "ef-side-trail" }, p.trail) : null);
  }

  function UsageBar(p) {
    var pct = Math.max(0, Math.min(100, p.value || 0));
    var color = p.color || (p.world ? "var(--world-" + p.world + ")" : "var(--accent)");
    if (pct >= 97) color = "var(--danger)"; else if (pct >= 90) color = "var(--warning)";
    return h("div", { className: cx("ef-usage", p.large && "ef-usage-lg"), role: "meter", "aria-valuenow": pct, "aria-valuemin": 0, "aria-valuemax": 100, "aria-label": p.label || "Used space" },
      h("i", { style: { width: pct + "%", background: color } }));
  }

  var STATE = {
    mounted: ["success", "Mounted"], readonly: ["warning", "Read-only"], dirty: ["warning", "Needs check"],
    locked: ["danger", "Locked"], unmounted: [null, "Not mounted"], mounting: ["accent", "Mounting…"],
    cloud: ["info", "Cloud-only"], ads: ["info", "+ADS"], hidden: [null, "Hidden"], system: [null, "System"]
  };
  function StatePill(p) {
    var s = STATE[p.state] || [p.tone, p.children];
    return h("span", { className: cx("ef-pill", s[0] && "ef-pill-" + s[0]) }, p.children || s[1]);
  }

  function DriveItem(p) {
    var off = p.state === "unmounted" || p.state === "locked";
    return h("button", { type: "button", className: cx("ef-drive", off && "ef-drive-off"), "aria-current": p.active ? "true" : undefined },
      h(Icon, { name: p.icon || "drive", size: 16 }),
      h("span", { className: "ef-drive-name" }, p.name),
      p.state === "mounting" ? h("span", { className: "ef-spinner", role: "status", "aria-label": "Mounting" }) :
        (p.state === "locked" ? h(Icon, { name: "lock", size: 14, className: "ef-drive-lock" }) : h("span")),
      !off && p.used !== undefined ? h(UsageBar, { value: p.used, world: p.world }) : null,
      h("span", { className: "ef-drive-meta" },
        h("span", { className: "ef-drive-meta-l" }, p.meta, p.state && p.state !== "mounted" && p.state !== "mounting" ? h(StatePill, { state: p.state }) : null),
        p.free ? h("span", null, p.free) : null));
  }

  // ------------------------------------------------------------ files
  function splitExt(name, kind) {
    if (kind === "folder") return [name, ""];
    var i = name.lastIndexOf(".");
    return i > 0 ? [name.slice(0, i), name.slice(i)] : [name, ""];
  }

  function FileRow(p) {
    var f = p.file;
    var parts = splitExt(f.name, f.kind);
    var name = p.renaming
      ? h("input", {
          className: "ef-rename", defaultValue: f.name, autoFocus: true, "aria-label": "New name",
          onFocus: function (e) { e.target.setSelectionRange(0, parts[0].length); }
        })
      : h("span", { className: "ef-file-name" }, parts[0], parts[1] ? h("span", { className: "ef-ext" }, parts[1]) : null);
    return h("div", {
      className: "ef-file", role: "row", "aria-selected": String(!!f.selected),
      "data-cursor": f.cursor ? "true" : undefined, "data-cut": f.cut ? "true" : undefined,
      "data-hidden": f.hidden ? "true" : undefined, "data-drop": f.drop ? "true" : undefined,
      "data-removing": f.removing ? "true" : undefined,
      onClick: p.onClick
    },
      h(FileIcon, { name: f.icon || iconFor(f.name, f.kind), size: 18, badge: f.badge }),
      name,
      h("span", { className: "ef-file-meta ef-file-num", role: "cell" }, f.size || (f.kind === "folder" ? (f.items !== undefined ? f.items + " items" : "—") : "")),
      h("span", { className: "ef-file-meta", role: "cell" }, f.type || ""),
      h("span", { className: "ef-file-meta", role: "cell" }, f.modified || ""));
  }

  function ColumnHeader(p) {
    var cols = [["name", "Name"], ["size", "Size"], ["type", "Kind"], ["modified", "Modified"]];
    return h("div", { className: "ef-list-head", role: "row" },
      h("span"),
      cols.map(function (c) {
        var on = p.sort === c[0];
        return h("button", { key: c[0], type: "button", role: "columnheader", className: cx("ef-colh", c[0] === "size" && "ef-colh-num"), "aria-sort": on ? (p.desc ? "descending" : "ascending") : undefined },
          c[1], on ? h(Icon, { name: p.desc ? "arrow-down" : "arrow-up", size: 12, strokeWidth: 2.5 }) : null);
      }));
  }

  function Skeleton(p) {
    var n = p.rows || 6, rows = [];
    for (var i = 0; i < n; i++) {
      rows.push(h("div", { className: "ef-skel", key: i, "aria-hidden": "true" },
        h("i"), h("i", { style: { width: (40 + ((i * 37) % 45)) + "%" } }), h("i", { style: { width: "70%", justifySelf: "end" } }), h("i", { style: { width: "60%" } }), h("i", { style: { width: "80%" } })));
    }
    return h("div", { role: "status", "aria-label": "Loading folder" }, rows);
  }

  function FileList(p) {
    var st = useState(p.cursor !== undefined ? p.cursor : -1);
    var files = p.files || [];
    return h("div", { className: cx("ef-list", p.density === "compact" && "ef-dense", p.density === "comfortable" && "ef-comfy"), role: "grid", "aria-label": p.label || "Files", "aria-multiselectable": "true", tabIndex: 0 },
      h(ColumnHeader, { sort: p.sort || "name", desc: p.desc }),
      p.loading ? h(Skeleton, { rows: p.loading }) : files.map(function (f, i) {
        return h(FileRow, { key: f.name, file: Object.assign({}, f, st[0] === i ? { cursor: true } : null), renaming: p.renaming === i, onClick: function () { st[1](i); } });
      }));
  }

  function FileTile(p) {
    var f = p.file;
    return h("div", { className: "ef-tile", role: "gridcell", "aria-selected": String(!!f.selected), "data-cursor": f.cursor ? "true" : undefined, tabIndex: -1 },
      h("div", { className: "ef-tile-art" },
        f.thumb ? h("img", { className: "ef-thumb", src: f.thumb, alt: "" }) : h(FileIcon, { name: f.icon || iconFor(f.name, f.kind), size: 48, badge: f.badge })),
      h("span", { className: "ef-tile-name", title: f.name }, f.name),
      f.sub ? h("span", { className: "ef-tile-sub" }, f.sub) : null);
  }
  function FileGrid(p) {
    return h("div", { className: "ef-grid", role: "grid", "aria-label": p.label || "Files" },
      (p.files || []).map(function (f) { return h(FileTile, { key: f.name, file: f }); }));
  }

  function StatusBar(p) {
    return h("footer", { className: "ef-status", role: "status" },
      h("span", null, h("strong", null, p.count), " items"),
      p.selected ? h("span", { className: "ef-status-sep" }) : null,
      p.selected ? h("span", null, h("strong", null, p.selected), " selected", p.selectedSize ? " · " + p.selectedSize : "") : null,
      h("span", { className: "ef-status-grow" }),
      p.task ? h("span", { style: { display: "inline-flex", alignItems: "center", gap: 6 } }, h("span", { className: "ef-spinner", style: { width: 10, height: 10, borderWidth: 1.5 } }), p.task) : null,
      p.task ? h("span", { className: "ef-status-sep" }) : null,
      p.volume ? h("span", null, p.volume) : null,
      p.state ? h(StatePill, { state: p.state }) : null,
      p.free ? h("span", null, p.free) : null);
  }

  function EmptyState(p) {
    return h("div", { className: "ef-empty" },
      h(FileIcon, { name: p.icon || "folder-open", size: 64 }),
      h("h3", null, p.title || "This folder is empty"),
      p.body ? h("p", null, p.body) : null,
      p.actions ? h("div", { className: "ef-empty-actions" }, p.actions) : null);
  }

  function DriveCard(p) {
    return h("button", { type: "button", className: "ef-dcard" },
      h(FileIcon, { name: p.icon || "drive", size: 48 }),
      h("span", { className: "ef-dcard-name" }, p.name, p.state && p.state !== "mounted" ? h(StatePill, { state: p.state }) : null),
      h("span", { className: "ef-dcard-sub" }, p.fs),
      p.used !== undefined ? h(UsageBar, { value: p.used, world: p.world, large: true }) : null,
      h("span", { className: "ef-dcard-free" }, h("span", null, p.free), h("span", null, p.total)));
  }

  // ------------------------------------------------------------ feedback
  function Spinner(p) { return h("span", { className: cx("ef-spinner", p.large && "ef-spinner-lg"), role: "status", "aria-label": p.label || "Loading" }); }

  function Banner(p) {
    var icon = p.icon || (p.tone === "warning" ? "alert" : p.tone === "danger" ? "error" : "info");
    return h("div", { className: cx("ef-banner", p.tone && "ef-banner-" + p.tone), role: p.tone === "danger" ? "alert" : "status" },
      h(Icon, { name: icon, size: 16 }),
      h("div", { className: "ef-banner-text" }, p.children),
      p.actions ? h("div", { className: "ef-banner-actions" }, p.actions) : null,
      p.onClose !== false ? h(IconButton, { icon: "close", label: "Dismiss" }) : null);
  }

  function Toast(p) {
    return h("div", { className: cx("ef-toast", p.tone && "ef-toast-" + p.tone), role: "status" },
      h(Icon, { name: p.icon || (p.tone === "success" ? "check" : p.tone === "danger" ? "error" : "info"), size: 16 }),
      h("span", { className: "ef-toast-title" }, p.title),
      h(IconButton, { icon: "close", label: "Dismiss" }),
      p.children ? h("div", { className: "ef-toast-body" }, p.children) : null,
      p.actions ? h("div", { className: "ef-toast-actions" }, p.actions) : null);
  }

  function TransferToast(p) {
    var st = useState(p.value || 0);
    var v = st[0];
    var done = v >= 100;
    useEffect(function () {
      if (!p.animate || done) return;
      var t = setInterval(function () { st[1](function (x) { return Math.min(100, x + 1.5); }); }, 90);
      return function () { clearInterval(t); };
    }, [done]);
    return h("div", { className: cx("ef-toast", done && "ef-toast-success"), role: "status", "aria-live": "polite" },
      h(Icon, { name: done ? "check" : (p.icon || "copy"), size: 16 }),
      h("span", { className: "ef-toast-title" }, done ? (p.doneTitle || "Copied") : p.title),
      done ? h(IconButton, { icon: "close", label: "Dismiss" }) : h("span", { style: { display: "flex", gap: 2 } }, h(IconButton, { icon: "pause", label: "Pause" }), h(IconButton, { icon: "close", label: "Cancel (Esc)" })),
      h("div", { className: "ef-toast-body" }, p.detail),
      h("div", { className: cx("ef-progress", done && "ef-progress-done", p.indeterminate && "ef-progress-indet"), role: "progressbar", "aria-valuenow": Math.round(v), "aria-valuemin": 0, "aria-valuemax": 100 },
        h("i", { style: { width: v + "%" } })),
      h("div", { className: "ef-toast-meta" },
        h("span", null, done ? (p.doneMeta || "Flushed to disk") : p.meta),
        h("span", null, done ? "" : p.eta)));
  }

  function Dialog(p) {
    return h("div", { className: "ef-scrim", style: p.inline === false ? null : { minHeight: p.height || 300 } },
      h("div", { className: cx("ef-dialog", p.tone === "danger" && "ef-dialog-danger"), role: "dialog", "aria-modal": "true", "aria-labelledby": "dlg-title" },
        h("div", { className: "ef-dialog-head" },
          p.icon ? h(FileIcon, { name: p.icon, size: 32 }) : null,
          h("div", { style: { flex: 1, minWidth: 0 } }, h("h2", { className: "ef-dialog-title", id: "dlg-title" }, p.title))),
        h("div", { className: "ef-dialog-body" }, p.children),
        h("div", { className: "ef-dialog-foot" }, p.footer)));
  }

  function ConflictDialog(p) {
    return h(Dialog, {
      title: p.title || "“Report-Q3.xlsx” already exists in AVS (D:)", icon: "file-sheet", height: 420,
      footer: [
        h("span", { key: "all", className: "ef-grow" }, h(Checkbox, { id: "apply-all" }, "Apply to all " + (p.count || 12) + " conflicts")),
        h(Button, { key: "skip" }, "Skip"),
        h(Button, { key: "both" }, "Keep both"),
        h(Button, { key: "rep", variant: "primary", kbd: ["Enter"] }, "Replace")]
    },
      h("div", { className: "ef-conflict" },
        h("div", { className: "ef-conflict-card" }, h(FileIcon, { name: "file-sheet", size: 32 }), h("b", null, "Copying"), h("span", { className: "ef-newer" }, "Modified today 18:22 · newer"), h("span", null, "84 KB · ~/Documents")),
        h("div", { className: "ef-conflict-card" }, h(FileIcon, { name: "file-sheet", size: 32 }), h("b", null, "Existing"), h("span", null, "Modified 2 Sep 09:10"), h("span", null, "81 KB · D:\\Work"))),
      h("p", null, "Keep both saves the copy as “Report-Q3 (2).xlsx”."));
  }

  function Tooltip(p) {
    return h("span", { className: "ef-tooltip", role: "tooltip" }, p.children, p.kbd ? h(Kbd, { keys: p.kbd }) : null);
  }

  function ContextMenu(p) {
    return h("div", { className: "ef-menu", role: "menu", "aria-label": p.label || "Actions" },
      (p.items || []).map(function (it, i) {
        if (it === "-") return h("div", { key: i, className: "ef-menu-sep", role: "separator" });
        if (it.heading) return h("div", { key: i, className: "ef-menu-head" }, it.heading);
        return h("button", { key: i, type: "button", role: "menuitem", className: cx("ef-mi", it.danger && "ef-mi-danger"), "data-active": it.active ? "true" : undefined, disabled: it.disabled },
          h(Icon, { name: it.icon || "chevron-right", size: 16 }),
          h("span", { className: "ef-mi-label" }, it.label),
          it.hint ? h("span", { className: "ef-mi-hint" }, it.hint) : null,
          it.kbd ? h(Kbd, { keys: it.kbd }) : null,
          it.submenu ? h(Icon, { name: "chevron-right", size: 12 }) : null);
      }));
  }

  function highlight(label, q) {
    if (!q) return label;
    var out = [], li = 0, qi = 0, lower = label.toLowerCase(), ql = q.toLowerCase();
    for (var i = 0; i < label.length; i++) {
      if (qi < ql.length && lower[i] === ql[qi]) { out.push(h("mark", { key: i }, label[i])); qi++; }
      else out.push(label[i]);
    }
    return out;
  }

  function CommandPalette(p) {
    var st = useState(p.query || "");
    var q = st[0];
    var groups = p.groups || [];
    return h("div", { className: "ef-palette", role: "dialog", "aria-label": "Command palette" },
      h("div", { className: "ef-palette-input" },
        h("span", { className: "ef-palette-prompt" }, ">"),
        h("input", { id: p.id || "palette-q", value: q, onChange: function (e) { st[1](e.target.value); }, placeholder: "Type a command, folder or drive…", "aria-label": "Command" }),
        h(Kbd, { keys: ["Esc"] })),
      h("div", { className: "ef-palette-list", role: "listbox" },
        groups.map(function (g, gi) {
          return h(React.Fragment, { key: gi },
            h("div", { className: "ef-menu-head" }, g.heading),
            g.items.map(function (it, i) {
              return h("button", { key: i, type: "button", role: "option", className: "ef-mi", "data-active": gi === 0 && i === 0 ? "true" : undefined, "aria-selected": gi === 0 && i === 0 ? "true" : undefined },
                h(Icon, { name: it.icon, size: 16 }),
                h("span", { className: "ef-mi-label" }, highlight(it.label, q)),
                it.hint ? h("span", { className: "ef-mi-hint" }, it.hint) : null,
                it.kbd ? h(Kbd, { keys: it.kbd }) : null);
            }));
        })),
      h("div", { className: "ef-palette-foot" },
        h("span", null, h(Kbd, { keys: ["↑", "↓"] }), "move"), h("span", null, h(Kbd, { keys: ["Enter"] }), "run"),
        h("span", null, h(Kbd, { keys: ["Tab"] }), "complete path"), h("span", null, h(Kbd, { keys: ["/"] }), "search files")));
  }

  // ------------------------------------------------------------ panels
  function PreviewPane(p) {
    var f = p.file || {};
    return h("aside", { className: "ef-preview", "aria-label": "Preview" },
      h("div", { className: "ef-preview-art" }, f.thumb ? h("img", { src: f.thumb, alt: "" }) : h(FileIcon, { name: f.icon || iconFor(f.name, f.kind), size: 96 })),
      h("div", { className: "ef-preview-body" },
        h("h2", { className: "ef-preview-title" }, f.name),
        h("dl", { className: "ef-props" }, (p.props || []).map(function (r, i) {
          return h(React.Fragment, { key: i }, h("dt", null, r[0]), h("dd", { title: r[1] }, r[1]));
        })),
        p.attrs ? h("div", null, h("div", { className: "ef-sec-head", style: { padding: 0, marginBottom: 6 } }, "Windows attributes"),
          h("div", { className: "ef-attrs" }, p.attrs.map(function (a) { return h(StatePill, { key: a, tone: null }, a); }))) : null,
        h("div", { className: "ef-preview-actions" },
          h(Button, { size: "sm", icon: "external" }, "Open"),
          h(Button, { size: "sm", icon: "copy" }, p.windowsPath ? "Copy Windows path" : "Copy path"))));
  }

  // ------------------------------------------------------------ pages
  var DEMO_FILES = [
    { name: "Projects", kind: "folder", items: 14, type: "Folder", modified: "Today 11:02" },
    { name: "Screenshots", kind: "folder", icon: "folder-image", items: 382, type: "Folder", modified: "Today 09:47" },
    { name: "Shortcut to Games", kind: "folder", badge: "link", type: "Junction", modified: "3 Aug" },
    { name: "Report-Q3.xlsx", size: "84 KB", type: "Spreadsheet", modified: "Today 18:22", selected: true },
    { name: "IMG_20260914_182233.jpg", size: "4.2 MB", type: "JPEG image", modified: "14 Sep 18:22", selected: true },
    { name: "Taxes-2025.pdf", size: "1.1 MB", type: "PDF", modified: "2 Sep", badge: "cloud" },
    { name: "main.rs", size: "12 KB", type: "Rust source", modified: "Yesterday" },
    { name: "Backup.7z", size: "2.4 GB", type: "7-Zip archive", modified: "21 Aug", badge: "ads" },
    { name: "desktop.ini", size: "282 B", type: "Settings", modified: "12 Jan", hidden: true },
    { name: "Old Launcher.lnk", size: "1 KB", type: "Shortcut", modified: "4 Mar", badge: "broken" }
  ];

  function DemoSidebar(p) {
    return h(Sidebar, null,
      h(SidebarSection, { title: "Linux", world: "linux" },
        h(SidebarItem, { icon: "home", label: "Home" }),
        h(SidebarItem, { icon: "file", label: "Documents" }),
        h(SidebarItem, { icon: "download", label: "Downloads", trail: "3 new" }),
        h(SidebarItem, { icon: "image", label: "Pictures" }),
        h(SidebarItem, { icon: "trash", label: "Trash" })),
      h(SidebarSection, { title: "Windows", world: "windows", count: 3 },
        h(DriveItem, { name: "Windows (C:)", state: "readonly", used: 76, world: "windows", meta: "NTFS", free: "88 GB free", active: p.active === "c" }),
        h(DriveItem, { name: "AVS (D:)", state: "mounted", used: 58, world: "windows", meta: "NTFS", free: "66 GB free", active: p.active === "d" }),
        h(DriveItem, { name: "AVS (E:)", state: "unmounted", meta: "295 GB · click to mount" })),
      h(SidebarSection, { title: "Pinned" },
        h(SidebarItem, { icon: "star", label: "Work", trail: "D:" }),
        h(SidebarItem, { icon: "folder", label: "EchoFiles_Linux" })));
  }

  function AppWindow(p) {
    return h("div", { className: "ef ef-app", style: { height: p.height || 640 } },
      h(TabStrip, { tabs: [{ label: "Work", icon: "folder" }, { label: "Downloads", icon: "download" }, { label: "Pictures", icon: "image" }], active: 0 }),
      h(Toolbar, { segments: [{ label: "AVS (D:)", icon: "drive" }, { label: "Work" }, { label: "2026" }], preview: true }),
      h("div", { className: "ef-app-main" },
        h(DemoSidebar, { active: "d" }),
        h("div", { className: "ef-app-center" },
          p.banner !== false ? h(Banner, { tone: "info", icon: "cloud", actions: h(Button, { size: "sm" }, "Learn more") },
            h("strong", null, "2 files are OneDrive placeholders."), " They open once Windows has synced them.") : null,
          h("div", { className: "ef-app-scroll" }, h(FileList, { files: DEMO_FILES, cursor: 4 }))),
        h(PreviewPane, {
          file: { name: "IMG_20260914_182233.jpg" },
          props: [["Kind", "JPEG image"], ["Size", "4.2 MB"], ["Dimensions", "4032 × 3024"], ["Modified", "14 Sep 2026 18:22"], ["Where", "D:\\Work\\2026"]],
          attrs: ["Archive"], windowsPath: true
        })),
      h(StatusBar, { count: "1,284", selected: 2, selectedSize: "4.3 MB", volume: "AVS (D:) · ntfs3", free: "66 GB free", task: "Copying 42%" }),
      h("div", { className: "ef-app-float" },
        h(TransferToast, { title: "Copying 1,204 files to AVS (D:)", detail: "From ~/Pictures/2026 · IMG_4471.HEIC", value: 42, animate: p.animate, meta: "812 MB of 2.4 GB · 94 MB/s", eta: "17 s left" })));
  }

  function DualPane(p) {
    var left = DEMO_FILES.slice(3, 8).map(function (f, i) { return Object.assign({}, f, { selected: i < 2, cursor: i === 1 }); });
    var right = DEMO_FILES.slice(0, 3).concat([{ name: "Report-Q3.xlsx", size: "81 KB", type: "Spreadsheet", modified: "2 Sep" }, { name: "Invoices", kind: "folder", items: 57, drop: true }]);
    function pane(title, icon, files, active, world) {
      return h("div", { className: cx("ef-pane", active ? "ef-pane-active" : "ef-pane-inactive") },
        h("div", { className: "ef-pane-head" },
          h("span", { className: "ef-sec-mark", style: { background: "var(--world-" + world + ")" } }),
          h(Icon, { name: icon, size: 14 }), h("strong", null, title), h("span", { className: "ef-muted", style: { marginLeft: "auto" } }, files.length + " items")),
        h("div", { className: "ef-list" },
          h("div", { className: "ef-list-head" }, h("span"), h("button", { className: "ef-colh", "aria-sort": "ascending", type: "button" }, "Name", h(Icon, { name: "arrow-up", size: 12 })), h("button", { className: "ef-colh ef-colh-num", type: "button" }, "Size")),
          files.map(function (f) {
            var parts = splitExt(f.name, f.kind);
            return h("div", { key: f.name, className: "ef-file", role: "row", "aria-selected": String(!!f.selected), "data-cursor": f.cursor ? "true" : undefined, "data-drop": f.drop ? "true" : undefined },
              h(FileIcon, { name: f.icon || iconFor(f.name, f.kind), size: 18, badge: f.badge }),
              h("span", { className: "ef-file-name" }, parts[0], parts[1] ? h("span", { className: "ef-ext" }, parts[1]) : null),
              h("span", { className: "ef-file-meta ef-file-num" }, f.size || (f.items + " items")));
          })));
    }
    return h("div", { className: "ef ef-app", style: { height: p.height || 420 } },
      h(Toolbar, { segments: [{ label: "Home", icon: "home" }, { label: "Documents" }], view: "columns" }),
      h("div", { className: "ef-dual" },
        pane("~/Documents", "home", left, true, "linux"),
        pane("AVS (D:) \\ Work", "drive", right, false, "windows")),
      h(StatusBar, { count: "5", selected: 2, selectedSize: "4.3 MB", volume: "F5 copy → AVS (D:)  ·  F6 move" }));
  }

  function MotionCell(p) {
    var st = useState(0);
    return h("div", { className: "ef-motion-cell" },
      h("div", { className: "ef-motion-stage", key: st[0] }, p.stage),
      h("b", null, p.title), h("span", null, p.note),
      h("button", { type: "button", className: "ef-btn ef-btn-sm ef-btn-ghost ef-replay", onClick: function () { st[1](st[0] + 1); } }, h(Icon, { name: "refresh", size: 12 }), "Replay"));
  }

  function MotionSpec() {
    var ghost = h("div", { className: "ef-ghost" },
      h(FileIcon, { name: "file-image", size: 36, className: "", key: 1 }),
      h("span", { style: { position: "absolute", left: 5, top: 5 } }, h(FileIcon, { name: "file-sheet", size: 36 })),
      h("span", { style: { position: "absolute", left: 10, top: 10 } }, h(FileIcon, { name: "file-pdf", size: 36 })),
      h("span", { className: "ef-ghost-count" }, "12"));
    return h("div", { className: "ef ef-motion" },
      h(MotionCell, { title: "Hover · 90ms", note: "State layer fades in. Never scales, never lifts.", stage: h(Button, null, "Hover me") }),
      h(MotionCell, { title: "Selection · 0ms", note: "Cursor and selection are instant. Lists never animate on keyboard input.", stage: h("div", { className: "ef-pane", style: { width: "94%", border: 0 } }, h(FileRow, { file: { name: "main.rs", size: "12 KB", selected: true, cursor: true } })) }),
      h(MotionCell, { title: "Breadcrumb · 140ms", note: "Navigation is instant; only the new segment slides in.", stage: h(PathBar, { segments: [{ label: "Home" }, { label: "Pictures" }], animateLast: true }) }),
      h(MotionCell, { title: "Drop target · spring", note: "Folder pops, fills accent-soft; spring-load bar opens it after 700ms.", stage: h("div", { className: "ef-pane", style: { width: "94%", border: 0 } }, h(FileRow, { file: { name: "Invoices", kind: "folder", items: 57, drop: true } })) }),
      h(MotionCell, { title: "Drag ghost", note: "Up to three icons stacked like the logo's sheets, plus a count.", stage: ghost }),
      h(MotionCell, { title: "Toast · 220ms spring", note: "Rises 12px. Exits with a 150ms fade.", stage: h("div", { className: "ef-motion-mini" }, h(Toast, { title: "Moved to Trash", tone: "success", actions: h(Button, { size: "sm" }, "Undo") })) }),
      h(MotionCell, { title: "Menu · 140ms", note: "Drops 4px from the pointer side.", stage: h("div", { className: "ef-motion-mini ef-motion-mini-menu" }, h(ContextMenu, { items: [{ icon: "external", label: "Open", active: true }, { icon: "rename", label: "Rename" }] })) }),
      h(MotionCell, { title: "Usage bar · 400ms", note: "Grows once when a drive mounts; then static.", stage: h("div", { style: { width: "80%" } }, h(UsageBar, { value: 64, world: "windows", large: true })) }),
      h(MotionCell, { title: "Skeleton · after 150ms", note: "Only shown if a folder takes longer than 150ms. Shimmer loops 1.4s.", stage: h("div", { className: "ef-pane", style: { width: "94%", border: 0 } }, h(Skeleton, { rows: 2 })) }),
      h(MotionCell, { title: "Delete · 140ms", note: "Row collapses with ease-exit; rows below slide up. Undo toast follows.", stage: h("div", { className: "ef-pane", style: { width: "94%", border: 0 } }, h(FileRow, { file: { name: "old-notes.txt", size: "2 KB", removing: true } })) }),
      h(MotionCell, { title: "Thumbnail · 140ms", note: "Decoded thumbnails fade over the icon. No layout shift.", stage: h("div", { className: "ef-tile-art", style: { width: 56, height: 56 } }, h("div", { className: "ef-thumb", style: { width: 56, height: 42, background: "linear-gradient(135deg, var(--world-windows), var(--success))" } })) }),
      h(MotionCell, { title: "Progress", note: "Linear fill; turns success and draws a tick at 100%.", stage: h("div", { className: "ef-motion-mini" }, h(TransferToast, { title: "Copying 3 files", detail: "To AVS (D:)", value: 70, animate: true, meta: "", eta: "" })) }));
  }

  window.Echo = {
    Icon: Icon, FileIcon: FileIcon, Button: Button, IconButton: IconButton, SegmentedControl: SegmentedControl,
    Switch: Switch, Checkbox: Checkbox, Kbd: Kbd, TextField: TextField, SearchField: SearchField, PathBar: PathBar,
    TabStrip: TabStrip, Toolbar: Toolbar, Sidebar: Sidebar, SidebarSection: SidebarSection, SidebarItem: SidebarItem,
    DriveItem: DriveItem, UsageBar: UsageBar, StatePill: StatePill, FileRow: FileRow, ColumnHeader: ColumnHeader,
    FileList: FileList, Skeleton: Skeleton, FileTile: FileTile, FileGrid: FileGrid, StatusBar: StatusBar,
    EmptyState: EmptyState, DriveCard: DriveCard, Spinner: Spinner, Banner: Banner, Toast: Toast,
    TransferToast: TransferToast, Dialog: Dialog, ConflictDialog: ConflictDialog, Tooltip: Tooltip,
    ContextMenu: ContextMenu, CommandPalette: CommandPalette, PreviewPane: PreviewPane, AppWindow: AppWindow,
    DualPane: DualPane, MotionSpec: MotionSpec, iconFor: iconFor, DEMO_FILES: DEMO_FILES
  };
})();
