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
    var scope = p.scope || "folder";
    return h("div", { className: "ef-toolbar", role: "toolbar", "aria-label": "Navigation" },
      h(IconButton, { icon: "arrow-left", label: "Back (Alt+Left)" }),
      h(IconButton, { icon: "arrow-right", label: "Forward (Alt+Right)", disabled: true }),
      h(IconButton, { icon: "arrow-up", label: "Parent folder (Alt+Up)" }),
      h(IconButton, { icon: "refresh", label: "Reload (F5)" }),
      h("span", { className: "ef-toolbar-sep" }),
      h(PathBar, { segments: p.segments || [], animateLast: true }),
      h(SearchScope, { value: scope }),
      h(SearchField, { placeholder: scope === "everywhere" ? "Search everywhere" : "Search this folder", defaultValue: p.query }),
      h("span", { className: "ef-toolbar-sep" }),
      h(ViewButton, { grid: !!p.grid, open: !!p.viewOpen }),
      h(IconButton, { icon: "command", label: "Command palette (Ctrl+K)" }),
      h(IconButton, { icon: "settings", label: "Settings (Ctrl+,)", pressed: !!p.settings }));
  }

  /** Toolbar View dropdown: the current layout's glyph plus a chevron; opens ViewMenu. */
  function ViewButton(p) {
    return h("button", { type: "button", className: "ef-ibtn ef-dropbtn", "aria-label": "View", title: "View", "aria-haspopup": "menu", "aria-expanded": String(!!p.open) },
      h(Icon, { name: p.grid ? "grid" : "list", size: 16 }),
      h(Icon, { name: "chevron-down", size: 12 }));
  }

  /** The View dropdown's menu: layout choice, then pane and hidden-file toggles; a check marks what's on. */
  function ViewMenu(p) {
    return h(ContextMenu, { label: "View", items: [
      { heading: "Layout" },
      { icon: "list", label: "List", kbd: ["Ctrl", "1"], checked: !p.grid },
      { icon: "grid", label: "Grid", kbd: ["Ctrl", "2"], checked: !!p.grid },
      "-",
      { heading: "Show" },
      { icon: "columns", label: "Dual pane", kbd: ["F3"], checked: !!p.dual },
      { icon: "sidebar", label: "Preview pane", kbd: ["Space"], checked: !!p.preview },
      { icon: "eye", label: "Hidden files", kbd: ["Ctrl", "H"], checked: !!p.hidden }] });
  }

  /** Where the search box looks: this folder, or everything the index covers (Ctrl+E). */
  function SearchScope(p) {
    return h(SegmentedControl, { label: "Search in", value: p.value || "folder", onChange: p.onChange, options: [
      { value: "folder", text: "Folder", title: "Search this folder (Ctrl+E)" },
      { value: "everywhere", text: "Everywhere", title: "Search everywhere (Ctrl+E)" }] });
  }

  function Sidebar(p) {
    return h("nav", { className: "ef-sidebar", "aria-label": "Places" }, p.children);
  }
  function SidebarSection(p) {
    var st = useState(!!p.defaultOpen);
    var open = p.collapsible ? st[0] : true;
    var headKids = [
      p.collapsible ? h(Icon, { key: "c", name: open ? "chevron-down" : "chevron-right", size: 12, className: "ef-sec-chev" }) : null,
      p.world ? h("span", { key: "m", className: "ef-sec-mark", style: { background: "var(--world-" + p.world + ")" } }) : null,
      h("span", { key: "t", className: "ef-sec-title" }, p.title),
      p.count !== undefined ? h("span", { key: "n", className: "ef-sec-count" }, p.count) : null,
      p.action ? h("span", { key: "a", className: "ef-sec-action", onClick: function (e) { e.stopPropagation(); } }, p.action) : null];
    var head = p.collapsible
      ? h("div", { className: "ef-sec-head ef-sec-fold", role: "button", tabIndex: 0, "aria-expanded": String(open), title: open ? "Collapse" : "Expand", onClick: function () { st[1](!open); } }, headKids)
      : h("div", { className: "ef-sec-head" }, headKids);
    // Collapsed, only children marked `keep` (the place you're in) stay.
    var kids = React.Children.toArray(p.children).filter(function (c) { return open || (c.props && c.props.keep); });
    return h("section", null, head, kids.length ? h("div", { className: "ef-sec-list" }, kids) : null);
  }
  function SidebarItem(p) {
    return h("button", { type: "button", className: cx("ef-side-item", p.dropTarget && "ef-side-drop", p.indent && "ef-side-indent"), "aria-current": p.active ? "true" : undefined },
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
    locked: ["danger", "Locked"], unmounted: [null, "Not mounted"], mounting: ["accent", "Mounting…"], connecting: ["info", "Connecting…"],
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
        h("span", { className: "ef-drive-meta-l" }, p.state && p.state !== "mounted" && p.state !== "mounting" ? h(StatePill, { state: p.state }) : null, p.meta),
        p.free ? h("span", null, p.free) : null));
  }

  // ------------------------------------------------------------ network
  function NetworkItem(p) {
    var on = p.state === "connected";
    return h("div", { className: cx("ef-net", !on && p.state !== "connecting" && "ef-net-off"), "aria-current": p.active ? "true" : undefined, role: "button", tabIndex: 0 },
      h(Icon, { name: p.protocol === "SMB" ? "network" : "server", size: 16, className: "ef-net-icon" }),
      h("span", { className: "ef-net-name" }, p.name),
      on ? h(IconButton, { icon: "arrow-up", label: "Disconnect" }) : h("span"),
      h("span", { className: "ef-net-meta" }, p.state === "connecting" ? h(StatePill, { state: "connecting" }) : (p.protocol + " · " + p.where)));
  }

  function ListRow(p) {
    return h("button", { type: "button", className: "ef-listrow" },
      h(Icon, { name: p.icon, size: 16 }),
      h("span", { className: "ef-listrow-text" }, h("span", null, p.title), h("span", { className: "ef-listrow-sub" }, p.sub)),
      h(Icon, { name: "chevron-right", size: 12 }));
  }

  var PROTOCOLS = [["SMB", "smb", "Windows share"], ["SFTP", "sftp", "SSH server"], ["FTP", "ftp", "FTP server"], ["FTPS", "ftps", "FTP server (TLS)"]];
  function ConnectDialog(p) {
    var st = useState(p.protocol || "SMB");
    var proto = PROTOCOLS.filter(function (x) { return x[0] === st[0]; })[0];
    var hasScheme = (p.value || "").indexOf("://") > 0;
    var busy = p.state === "connecting";
    return h(Dialog, {
      title: "Connect to server", icon: "network", height: p.height || 560,
      footer: [
        h(Button, { key: "c", variant: "ghost", kbd: ["Esc"] }, "Cancel"),
        h(Button, { key: "g", variant: "primary", icon: busy ? "sync" : "network", kbd: busy ? null : ["Enter"], disabled: busy || p.parsed === "error" }, busy ? "Connecting…" : "Connect")]
    },
      h("div", { className: "ef-row-flex" },
        h(SegmentedControl, { label: "Protocol", value: st[0], onChange: st[1], options: PROTOCOLS.map(function (x) { return { value: x[0], text: x[0] }; }) }),
        h("span", { className: "ef-muted" }, proto[2])),
      h("div", { className: "ef-connect-field" },
        hasScheme ? null : h("span", { className: "ef-scheme" }, proto[1] + "://"),
        h("div", { className: cx("ef-field", (p.parsed === "error" || p.error) && "ef-field-invalid") }, h("input", { defaultValue: p.value || "", placeholder: "nas.local/Media   or   \\\\nas\\Media", "aria-label": "Server address" }))),
      p.parsed === "ok" ? h("div", { className: "ef-parse ef-parse-ok" }, h(Icon, { name: "check", size: 14 }), p.describe || "Windows share “Media” on nas.local") :
        p.parsed === "error" ? h("div", { className: "ef-parse ef-parse-warn" }, h(Icon, { name: "alert", size: 14 }), p.describe || "The address needs a server name or IP") :
          h("div", { className: "ef-parse" }, "Type a server name or IP, or paste an address like smb://, sftp:// or \\\\server\\share."),
      h(Checkbox, { defaultChecked: true }, "Add to sidebar"),
      h("div", { className: "ef-connect-lists" },
        (p.recent || []).length ? h("div", { className: "ef-group-head" }, "Recent") : null,
        (p.recent || []).map(function (r, i) { return h(ListRow, { key: "r" + i, icon: r[2] || "network", title: r[0], sub: r[1] }); }),
        h("div", { className: "ef-group-head" }, "On this network", h(IconButton, { icon: "refresh", label: "Look again" })),
        (p.nearby || []).length ? (p.nearby || []).map(function (r, i) { return h(ListRow, { key: "n" + i, icon: r[2] || "network", title: r[0], sub: r[1] }); }) :
          h("div", { className: "ef-muted", style: { fontSize: 12 } }, "No servers are announcing themselves here. Type an address above.")),
      p.error ? h("div", { className: "ef-inline-error", role: "alert" }, h(Icon, { name: "error", size: 14 }), p.error) : null);
  }

  function SignInDialog(p) {
    var st = useState(!!p.guest);
    var guest = st[0];
    return h(Dialog, {
      title: p.title || "Sign in to Media on nas.local", icon: "lock", height: p.height || 470,
      footer: [
        h(Button, { key: "c", variant: "ghost", kbd: ["Esc"] }, "Cancel"),
        h(Button, { key: "s", variant: "primary", icon: "key", kbd: ["Enter"] }, guest ? "Connect as guest" : "Sign in")]
    },
      h("p", null, p.detail || "Enter user and password for share “media” on “nas.local”"),
      p.anonymous === false ? null : h(SegmentedControl, { label: "Account", value: guest ? "guest" : "user", onChange: function (v) { st[1](v === "guest"); }, options: [{ value: "user", text: "Registered user" }, { value: "guest", text: "Guest" }] }),
      guest ? h("p", { className: "ef-muted" }, "Connect without an account. Guests usually see only public folders.") :
        h("div", { className: "ef-stack" },
          h("div", { className: "ef-signin-who" },
            h(TextField, { id: "si-user", label: "User name", defaultValue: p.user || "alice" }),
            p.domain === false ? null : h(TextField, { id: "si-domain", label: "Domain", defaultValue: "WORKGROUP", className: "ef-signin-domain" })),
          h(TextField, { id: "si-pass", label: "Password", type: "password", className: p.retry ? "ef-signin-retry" : null }),
          h(Checkbox, null, "Remember password in the keyring")),
      p.retry ? h("div", { className: "ef-parse ef-parse-err" }, h(Icon, { name: "error", size: 14 }), "That didn't work. Check the user name and password and try again.") : null);
  }

  function SharesPage(p) {
    var shares = p.shares || [["Media", "connected"], ["Photos"], ["Backups"], ["Public"]];
    return h("div", { className: "ef-shares" },
      h("h1", { className: "ef-shares-title" }, "Shares on " + (p.host || "nas.local")),
      h("p", { className: "ef-muted" }, "Pick a share to open it. It's added to the Network section while it's connected."),
      p.state === "loading" ? h("div", { className: "ef-row-flex" }, h(Spinner, null), h("span", { className: "ef-muted" }, "Asking the server for its shares…")) :
        h("div", { className: "ef-shares-grid" }, shares.map(function (s) {
          return h("button", { key: s[0], type: "button", className: "ef-share" },
            h(FileIcon, { name: "folder-share", size: 40 }),
            h("span", { className: "ef-share-name" }, s[0]),
            s[1] === "connected" ? h(StatePill, { tone: "success" }, "Connected") : s[1] === "connecting" ? h(StatePill, { tone: "info" }, "Connecting…") : h("span", { className: "ef-muted" }, "Windows share"));
        })));
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
      p.text ? h("span", null, p.text) : h("span", null, h("strong", null, p.count), " items"),
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
        h("span", { key: "all", className: "ef-grow" }, h(Checkbox, { id: "apply-all" }, "Do this for all " + (p.count || 12) + " conflicts")),
        h(Button, { key: "cancel", variant: "ghost", kbd: ["Esc"] }, "Cancel"),
        h(Button, { key: "skip" }, "Skip"),
        h(Button, { key: "rep" }, p.folders ? "Merge" : "Replace"),
        h(Button, { key: "both", variant: "primary", kbd: ["Enter"] }, "Keep both")]
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
          it.submenu ? h(Icon, { name: "chevron-right", size: 12 }) : null,
          it.checked !== undefined ? h("span", { className: "ef-mi-check" }, it.checked ? h(Icon, { name: "check", size: 14 }) : null) : null);
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
      h(SidebarSection, { title: "Pinned" },
        h(SidebarItem, { icon: "pin", label: "Work" }),
        h(SidebarItem, { icon: "pin", label: "EchoFiles_Linux" })),
      h(SidebarSection, { title: "Windows", world: "windows", count: 3, collapsible: true, defaultOpen: true },
        h(DriveItem, { name: "Windows (C:)", state: "readonly", used: 76, world: "windows", meta: "NTFS", free: "88 GB free", active: p.active === "c" }),
        h(DriveItem, { name: "AVS (D:)", state: "mounted", used: 58, world: "windows", meta: "NTFS", free: "66 GB free", active: p.active === "d" }),
        h(DriveItem, { name: "AVS (E:)", state: "unmounted", meta: "295 GB · click to mount" }),
        h(SidebarItem, { icon: "sliders", label: "All drives" })),
      h(SidebarSection, { title: "Phone", world: "phone", collapsible: true, defaultOpen: true },
        h(PhoneItem, { state: p.phone || "connected", name: "Galaxy S24", battery: 72, charging: true, active: !!p.phone && (p.phoneView || "hub") === "hub", keep: true }),
        p.phone === "none" || p.phone === "away" ? null : h(SidebarItem, { icon: "folder", label: "Files", indent: true }),
        p.phone === "none" || p.phone === "away" ? null : h(SidebarItem, { icon: "image", label: "Photos", indent: true, trail: "32 new", active: p.phoneView === "photos" })),
      h(SidebarSection, { title: "Network", world: "network", count: 1, collapsible: true, action: h(IconButton, { icon: "plus", label: "Connect to server (Ctrl+Shift+S)" }) },
        h(NetworkItem, { name: "Media", protocol: "SMB", where: "nas.local", state: "connected", keep: true }),
        h(NetworkItem, { name: "build-box", protocol: "SFTP", where: "me@build-box" }),
        h(SidebarItem, { icon: "plus", label: "Connect to server…" })));
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

  // ------------------------------------------------------------ settings
  // ------------------------------------------------------------ settings
  /** Uppercase label above a bordered box of rows separated by hairlines. */
  function SettingsGroup(p) {
    return h("section", { className: "ef-setg" },
      h("h3", { className: "ef-setg-label" }, p.title),
      h("div", { className: "ef-setbox" }, p.children));
  }

  /** One setting: what it does on the left, its control on the right. */
  function SettingRow(p) {
    return h("div", { className: cx("ef-setrow", p.disabled && "ef-setrow-off") },
      h("div", { className: "ef-setrow-text" },
        h("span", { className: "ef-setrow-label" }, p.label),
        p.description ? h("span", { className: "ef-setrow-desc" }, p.description) : null),
      h("div", { className: "ef-setrow-control" }, p.children));
  }

  /** A full-width row inside a group: notes, lists, code. */
  function SettingBlock(p) {
    return h("div", { className: cx("ef-setblock", p.muted && "ef-setrow-desc") }, p.children);
  }

  function PathListEditor(p) {
    var st = useState(p.paths || []);
    var input = useState(p.draft || "");
    var paths = st[0];
    return h(React.Fragment, null,
      paths.length === 0 ? h(SettingBlock, { muted: true }, p.empty || "Nothing here yet.") :
        paths.map(function (path, i) {
          return h("div", { key: path, className: "ef-plist-row" },
            h(Icon, { name: p.icon || "folder", size: 16 }),
            h("span", { className: "ef-plist-path", title: path }, path),
            p.note && p.note[i] ? h(StatePill, { tone: "warning" }, p.note[i]) : null,
            h(IconButton, { icon: "close", label: "Remove " + path, onClick: function () { st[1](paths.filter(function (_, k) { return k !== i; })); } }));
        }),
      h("div", { className: "ef-setblock" },
        h("div", { className: "ef-plist-add" },
          h("div", { className: cx("ef-field", p.error && "ef-field-invalid"), style: { flex: 1 } },
            h("input", { id: p.id, value: input[0], placeholder: p.placeholder || "Folder path, like ~/Projects", "aria-label": "Folder to add", "aria-invalid": p.error ? "true" : undefined, onChange: function (e) { input[1](e.target.value); } })),
          h(Button, { icon: "plus", onClick: function () { if (input[0].trim()) { st[1](paths.concat([input[0].trim()])); input[1](""); } } }, "Add"),
          p.current ? h(Button, { variant: "ghost", icon: "folder-plus", onClick: function () { st[1](paths.concat([p.current])); } }, "Add current folder") : null),
        p.error ? h("div", { className: "ef-field-msg", role: "alert" }, h(Icon, { name: "error", size: 14 }), p.error) : null));
  }

  function NameChips(p) {
    var st = useState(p.names || []);
    var input = useState("");
    var names = st[0];
    function add() { if (input[0].trim()) { st[1](names.concat([input[0].trim()])); input[1](""); } }
    return h(React.Fragment, null,
      h(SettingBlock, null, h("div", { className: "ef-chips" },
        names.map(function (n, i) {
          return h("span", { key: n, className: "ef-namechip" }, n,
            h("button", { type: "button", "aria-label": (p.reset === false ? "Remove " : "Stop skipping ") + n, onClick: function () { st[1](names.filter(function (_, k) { return k !== i; })); } }, h(Icon, { name: "close", size: 12 })));
        }))),
      h(SettingBlock, null, h("div", { className: "ef-plist-add" },
        h("div", { className: "ef-field", style: { flex: 1 } },
          h("input", { id: p.id, value: input[0], placeholder: p.placeholder || "Folder name, like node_modules", "aria-label": p.inputLabel || "Folder name to skip", onChange: function (e) { input[1](e.target.value); }, onKeyDown: function (e) { if (e.key === "Enter") add(); } })),
        h(Button, { icon: "plus", onClick: add }, "Add"),
        p.reset === false ? null : h(Button, { variant: "ghost", icon: "undo" }, "Reset to recommended"))));
  }

  var INDEX = {
    off: [null, "Off", "Everywhere search walks your folders live instead — slower on big folders."],
    opening: ["info", "Opening", "Loading the saved index…"],
    building: ["info", "Indexing", "Building the index for the first time. Search works live meanwhile."],
    updating: ["info", "Updating", "179,581 items · 13 MB · updating now"],
    ready: ["success", "Ready", "179,581 items · 13 MB on disk · updated 21 s ago"],
    problem: ["warning", "Problem", "Couldn't index ~/Work: permission denied."]
  };
  /** Health of the search index, with the one action that helps. */
  function IndexStatus(p) {
    var s = INDEX[p.state || "ready"];
    var busy = p.state === "building" || p.state === "updating";
    return h("div", { className: "ef-idx" },
      h(StatePill, { tone: s[0] }, s[1]),
      h("span", { className: "ef-idx-detail" }, p.detail || s[2]),
      h(Button, { icon: "refresh", disabled: busy || p.state === "off" }, busy ? "Indexing…" : "Rebuild now"));
  }

  /** Commands or shortcuts with what they do, on the bg-deep well. */
  function CommandList(p) {
    return h("div", { className: "ef-cmds" }, (p.items || []).map(function (it, i) {
      return h("div", { key: i, className: "ef-cmd" }, h("code", null, it[0]), h("span", null, it[1]));
    }));
  }

  var SETTINGS_PAGES = [
    { id: "general", icon: "sliders", label: "General" },
    { id: "search", icon: "search", label: "Search & index" },
    { id: "agents", icon: "terminal", label: "AI agents" },
    { id: "phone", icon: "phone", label: "Phone" },
    { id: "appearance", icon: "image", label: "Appearance" },
    { id: "about", icon: "info", label: "About" }];

  function SettingsNav(p) {
    return h("nav", { className: "ef-sidebar ef-setnav", "aria-label": "Settings pages" },
      SETTINGS_PAGES.map(function (pg) {
        return h("button", { key: pg.id, type: "button", className: "ef-side-item ef-setnav-item", "aria-current": pg.id === p.page ? "true" : undefined, onClick: function () { p.onChange && p.onChange(pg.id); } },
          h(Icon, { name: pg.icon, size: 16 }), h("span", { className: "ef-side-name" }, pg.label));
      }));
  }

  /** Settings as its own screen: top bar back to Files, page nav, one page of groups. */
  function SettingsShell(p) {
    var saved = p.saved !== false;
    return h("div", { className: "ef ef-settings", style: { height: p.height } },
      h("div", { className: "ef-toolbar ef-settings-bar" },
        h(Button, { variant: "ghost", icon: "arrow-left", kbd: ["Esc"] }, "Files"),
        h("span", { className: "ef-toolbar-sep" }),
        h("strong", { className: "ef-settings-title" }, "Settings"),
        h("span", { className: "ef-status-grow" }),
        h("span", { className: saved ? "ef-saved" : "ef-muted" }, h(Icon, { name: saved ? "check" : "clock", size: 14 }), saved ? "Saved" : "Saving…")),
      h("div", { className: "ef-settings-body" },
        h(SettingsNav, { page: p.page, onChange: p.onPage }),
        h("div", { className: "ef-settings-scroll" }, h("div", { className: "ef-settings-inner" }, p.children))));
  }

  function PageHead(p) {
    return h("header", { className: "ef-settings-head" }, h("h2", null, p.title), h("p", null, p.children));
  }

  function settingsPage(page) {
    if (page === "phone") return phoneSettings({});
    if (page === "general") return [
      h(PageHead, { key: "h", title: "General" }, "How EchoFiles starts, runs and opens."),
      h(SettingsGroup, { key: "a", title: "Running" },
        h(SettingRow, { label: "Keep running in the background", description: "Closing the window hides EchoFiles instead of quitting. The next window opens instantly and search stays up to date." }, h(Switch, { defaultChecked: true, label: "Keep running in the background" })),
        h(SettingRow, { label: "Start at login", description: "Starts hidden when you log in, so even the first window is instant." }, h(Switch, { label: "Start at login" }))),
      h(SettingsGroup, { key: "b", title: "Windows" },
        h(SettingRow, { label: "New windows open at", description: "Where EchoFiles starts when you open it without a folder." }, h(SegmentedControl, { label: "Open at", value: "home", options: [{ value: "home", text: "Home" }, { value: "last", text: "Last folder" }] })),
        h(SettingRow, { label: "Show hidden files", description: "Dotfiles and dot-folders. Ctrl+H switches it for the open window." }, h(Switch, { label: "Show hidden files" })))];
    if (page === "agents") return [
      h(PageHead, { key: "h", title: "AI agents" }, "Let AI agents on this computer use EchoFiles' index. Nothing leaves your computer."),
      h(SettingsGroup, { key: "a", title: "Command line" },
        h(SettingRow, { label: "Allow EchoFiles commands", description: "Lets AI agents and scripts search your files with ef find — milliseconds instead of walking the disk. When off, ef refuses and says it's turned off." }, h(Switch, { defaultChecked: true, label: "Allow EchoFiles commands" })),
        h(SettingRow, { label: "ef command", description: "Installed on your PATH." }, h("span", { className: "ef-muted" }, "~/.local/bin/ef"))),
      h(SettingsGroup, { key: "b", title: "Skill" },
        h(SettingRow, { label: "Teach AI agents about ef", description: "Links the EchoFiles skill into ~/.claude/skills so agents like Claude Code reach for ef before find. Turning it off removes only that link." }, h(Switch, { label: "Teach AI agents about ef" }))),
      h(SettingsGroup, { key: "c", title: "Commands" }, h(SettingBlock, null, h(CommandList, { items: [
        ["ef find report", "names containing “report”"], ["ef find '*' --ext pdf --limit 50", "every PDF, first 50"],
        ["ef find invoice --in ~/Documents", "only inside a folder"], ["ef status", "what's indexed, how fresh"], ["ef index", "update the index now"]] })))];
    if (page === "appearance") return [
      h(PageHead, { key: "h", title: "Appearance" }, "EchoFiles looks like the rest of your desktop."),
      h(SettingsGroup, { key: "a", title: "Theme" },
        h(SettingRow, { label: "Omarchy theme · tokyo-night", description: "EchoFiles follows your Omarchy theme and font and switches with them. Change the theme from the Omarchy menu." },
          h("span", { className: "ef-swatches" }, ["--bg", "--bg-raised", "--ink", "--accent", "--world-linux", "--world-windows", "--success", "--warning", "--danger"].map(function (v) { return h("i", { key: v, style: { background: "var(" + v + ")" } }); })))),
      h(SettingsGroup, { key: "b", title: "Layout" },
        h(SettingRow, { label: "Row height", description: "How tightly the file list is packed." }, h(SegmentedControl, { label: "Row height", value: "default", options: [{ value: "compact", text: "Compact" }, { value: "default", text: "Default" }, { value: "comfortable", text: "Comfortable" }] })),
        h(SettingRow, { label: "Folders open as", description: "Automatic uses a grid in Pictures, Videos and camera folders and a list everywhere else. Changing this resets views you switched by hand (Ctrl+1 / Ctrl+2)." }, h(SegmentedControl, { label: "Folders open as", value: "auto", options: [{ value: "auto", text: "Automatic" }, { value: "list", text: "List" }, { value: "grid", text: "Grid" }] }))),
      h(SettingsGroup, { key: "c", title: "Sidebar" },
        h(SettingRow, { label: "Windows drives", description: "The Windows section with your NTFS and BitLocker drives. Click its heading to open or close it." }, h(Switch, { label: "Windows drives", defaultChecked: true })),
        h(SettingRow, { label: "Network places", description: "The Network section with Windows shares, SSH and FTP servers. Ctrl+Shift+S connects to a server either way." }, h(Switch, { label: "Network places", defaultChecked: true })),
        h(SettingRow, { label: "Phone", description: "The Phone section with your paired phone, its files and photos. Pairing stays when it's hidden." }, h(Switch, { label: "Phone", defaultChecked: true })))];
    if (page === "about") return [
      h(PageHead, { key: "h", title: "About" }, "Where EchoFiles keeps things, and how to drive it from the keyboard."),
      h(SettingsGroup, { key: "a", title: "EchoFiles" },
        h(SettingRow, { label: "Version", description: "EchoFiles 0.1.0 · Linux" }),
        h(SettingRow, { label: "Settings file", description: "~/.config/echofiles/settings.toml" }, h(Button, { variant: "ghost", icon: "folder" }, "Show")),
        h(SettingRow, { label: "Index files", description: "~/.cache/echofiles/index" }, h(Button, { variant: "ghost", icon: "folder" }, "Show"))),
      h(SettingsGroup, { key: "b", title: "Keyboard" }, h(SettingBlock, null, h(CommandList, { items: [
        ["Ctrl+F  /", "search"], ["Ctrl+E", "search this folder ↔ everywhere"], ["Esc", "clear search · leave Settings"],
        ["Alt+← Alt+→ Alt+↑", "back · forward · parent"], ["Ctrl+H", "show hidden files"], ["F5", "reload"], ["Ctrl+,", "settings"]] })))];
    return [
      h(PageHead, { key: "h", title: "Search & index" }, "What search covers, and the index that makes it instant."),
      h(SettingsGroup, { key: "a", title: "Index" },
        h(SettingRow, { label: "Search index", description: "A list of file names kept on disk so Everywhere search answers instantly and AI agents can use ef. Updates within seconds in Downloads, Desktop and Documents and every minute elsewhere. Turning it off deletes the index." }, h(Switch, { defaultChecked: true, label: "Search index" })),
        h(IndexStatus, { state: "ready" })),
      h(SettingsGroup, { key: "b", title: "Search box" },
        h(SettingRow, { label: "Search looks in", description: "What the search box searches when a window opens. Ctrl+E switches it any time." }, h(SegmentedControl, { label: "Default scope", value: "folder", options: [{ value: "folder", text: "This folder" }, { value: "everywhere", text: "Everywhere" }] }))),
      h(SettingsGroup, { key: "c", title: "Indexed folders" },
        h(SettingBlock, { muted: true }, "Everywhere search and ef cover these folders and everything inside them."),
        h(PathListEditor, { id: "roots", paths: ["~"], current: "~/Projects/EchoFiles_Linux" })),
      h(SettingsGroup, { key: "d", title: "Never show in search" },
        h(SettingBlock, { muted: true }, "Folders that never appear in search results — private or noisy places. Their contents aren't indexed at all."),
        h(PathListEditor, { id: "excl", paths: [], empty: "Nothing excluded.", placeholder: "Folder path, like ~/Private", current: "~/Projects/EchoFiles_Linux" })),
      h(SettingsGroup, { key: "e", title: "Skip contents of" },
        h(SettingRow, { label: "Skip cache folders", description: "Folders marked with CACHEDIR.TAG (browser, build and package caches). They're still listed by name." }, h(Switch, { defaultChecked: true, label: "Skip cache folders" })),
        h(SettingBlock, { muted: true }, "Folders with these names are listed but their contents aren't indexed — package stores, build output, version control internals."),
        h(NameChips, { id: "names", names: ["node_modules", ".git", ".hg", ".svn", "__pycache__", ".cache", ".npm", ".pnpm-store", ".yarn", ".gradle", ".m2", ".cargo", ".rustup", ".venv", ".tox", ".mypy_cache", ".pytest_cache", ".next", ".nuxt"] }))];
  }

  function SettingsPage(p) {
    var st = useState(p.page || "search");
    return h(SettingsShell, { height: p.height, saved: p.saved, page: st[0], onPage: st[1] }, settingsPage(st[0]));
  }

  // ------------------------------------------------------------ phone
  var phoneUid = 0;
  var PHONE_SANS = "Inter, 'SF Pro Display', Roboto, 'Segoe UI', system-ui, sans-serif";

  /** A small battery that fills to `level`, green when charging, warning at 20% and below. */
  function BatteryMeter(p) {
    var lv = Math.max(0, Math.min(100, p.level || 0));
    var tone = p.charging ? "var(--success)" : lv <= 10 ? "var(--danger)" : lv <= 20 ? "var(--warning)" : "currentColor";
    return h("span", { className: "ef-batt", role: "meter", "aria-label": "Battery " + lv + "%" + (p.charging ? ", charging" : ""), "aria-valuenow": lv },
      h("span", { className: "ef-batt-body" }, h("i", { style: { width: lv + "%", background: tone } })),
      p.charging ? h(Icon, { name: "bolt", size: 11, className: "ef-batt-bolt" }) : null,
      p.label === false ? null : h("span", { className: "ef-batt-num" }, lv + "%"));
  }

  /** Stand-in photos: small painted scenes, seeded so every render shows the same picture. */
  var SCENES = [
    ["#ff9a5a", "#c2477d", "#2b1846", "#ffd27a"], ["#7cc6ff", "#3b6fd8", "#13285c", "#fff6c9"], ["#ffcf8a", "#ff7b54", "#3a1f3d", "#fff1c1"],
    ["#a7e3c4", "#3d9a7a", "#0f3b36", "#f4ffd8"], ["#c9b6ff", "#6a4cd6", "#1d1647", "#ffe0f2"], ["#ffd6e0", "#e0637f", "#41203a", "#fff5e0"],
    ["#9fd8ff", "#2d8bc9", "#0b2f4f", "#ffffff"], ["#ffe08a", "#f0883e", "#4a2512", "#fff8d6"]];
  function photoSrc(i, kind) {
    var s = SCENES[i % SCENES.length], svg;
    if (kind === "screenshot") {
      svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 90 160"><rect width="90" height="160" fill="#101320"/><rect x="6" y="10" width="40" height="5" rx="2" fill="#e8ecff"/><rect x="6" y="22" width="78" height="34" rx="5" fill="' + s[1] + '"/>' +
        '<rect x="6" y="62" width="78" height="10" rx="3" fill="#232842"/><rect x="6" y="76" width="60" height="10" rx="3" fill="#232842"/><rect x="6" y="90" width="70" height="10" rx="3" fill="#232842"/><rect x="6" y="140" width="78" height="12" rx="6" fill="' + s[0] + '"/></svg>';
    } else if (kind === "portrait") {
      svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="g" x2="0" y2="1"><stop offset="0" stop-color="' + s[0] + '"/><stop offset="1" stop-color="' + s[1] + '"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/>' +
        '<circle cx="50" cy="42" r="16" fill="' + s[2] + '" opacity=".85"/><path d="M18 100c4-22 18-32 32-32s28 10 32 32z" fill="' + s[2] + '" opacity=".85"/></svg>';
    } else {
      svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" preserveAspectRatio="xMidYMid slice"><defs><linearGradient id="g" x2="0" y2="1"><stop offset="0" stop-color="' + s[0] + '"/><stop offset=".7" stop-color="' + s[1] + '"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/>' +
        '<circle cx="' + (30 + (i * 17) % 45) + '" cy="40" r="11" fill="' + s[3] + '" opacity=".9"/><path d="M0 70 22 50l14 12 20-22 22 24 22-14v50H0z" fill="' + s[2] + '"/><path d="M0 82 30 68l26 10 22-8 22 8v22H0z" fill="#000" opacity=".28"/></svg>';
    }
    return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
  }

  /** The phone, drawn flat like the colour icons: a flat-sided Android phone in theme slot colours, thin even bezels, a pin-hole camera and a One UI–style lock screen. */
  function PhoneDevice(p) {
    var id = useState(function () { return "ph" + (++phoneUid); })[0];
    var w = p.width || 220, state = p.state || "connected";
    var off = state === "away";
    var lv = p.battery === undefined ? 72 : p.battery;
    var u = function (n) { return "url(#" + id + n + ")"; };
    var white = "#ffffff";
    var time = (p.time || "10:42").split(":");
    var X = 14, Y = 14, W = 276, H = 592, R = 30; // screen
    var wall = p.wallpaper === "photo"
      ? [h("image", { key: "img", href: photoSrc(p.photo || 0), x: X, y: Y, width: W, height: H, preserveAspectRatio: "xMidYMid slice" })]
      : [h("rect", { key: "b", x: X, y: Y, width: W, height: H, fill: "#0b0d14" }),
        h("circle", { key: "c1", cx: 300, cy: 300, r: 110, style: { fill: "var(--world-network)" }, opacity: 0.5 }),
        h("circle", { key: "c2", cx: 10, cy: 520, r: 120, style: { fill: "var(--accent)" }, opacity: 0.35 }),
        h("circle", { key: "c3", cx: 260, cy: 620, r: 130, style: { fill: "var(--world-linux)" }, opacity: 0.45 })];
    var font = { fontFamily: PHONE_SANS };
    var screen = off
      ? [h("rect", { key: "off", x: X, y: Y, width: W, height: H, fill: "#040506" }),
        h("text", Object.assign({ key: "t", x: 152, y: 300, textAnchor: "middle", fill: "#4d5262", fontSize: 14 }, font), "Not nearby")]
      : wall.concat([
        // status bar: time + notification icons left, signal / wifi / battery right
        h("text", Object.assign({ key: "st", x: 34, y: 40, fill: white, fontSize: 12.5, fontWeight: 600 }, font), time.join(":")),
        h("g", { key: "ni", fill: white, fillOpacity: 0.85 },
          h("circle", { cx: 80, cy: 36, r: 3.2 }), h("rect", { x: 88, y: 32.5, width: 7, height: 7, rx: 1.5 })),
        h("g", { key: "sb", transform: "translate(206 29)", fill: white, stroke: "none" },
          h("path", { d: "M0 11h2v2H0zM3.5 8h2v5h-2zM7 5h2v8H7zM10.5 2h2v11h-2z" }),
          h("path", { d: "M17 6.2a9 9 0 0 1 12 0l-6 7z", fillOpacity: 0.95 }),
          h("text", Object.assign({ x: 33, y: 11.5, fontSize: 11, fontWeight: 600 }, font), lv + "%"),
          h("rect", { x: 57, y: 2, width: 8, height: 12, rx: 2, fill: "none", stroke: white, strokeWidth: 1.2 }),
          h("rect", { x: 59, y: 2 + 2 + 8 * (1 - lv / 100), width: 4, height: 8 * lv / 100, rx: 0.8 })),
        // One UI clock: date and weather above, hours stacked over minutes, bold
        h("text", Object.assign({ key: "dt", x: 36, y: 96, fill: white, fillOpacity: 0.9, fontSize: 14, fontWeight: 500 }, font), p.date || "Fri 2 October  ·  24°"),
        h("text", Object.assign({ key: "hh", x: 30, y: 186, fill: white, fontSize: 96, fontWeight: 700, letterSpacing: -4 }, font), time[0]),
        h("text", Object.assign({ key: "mm", x: 30, y: 274, fill: white, fillOpacity: 0.92, fontSize: 96, fontWeight: 700, letterSpacing: -4 }, font), time[1]),
        state === "ringing"
          ? h("g", { key: "ring" },
            h("rect", { x: 26, y: 380, width: 252, height: 92, rx: 26, fill: "#0b0f24", fillOpacity: 0.62 }),
            h("circle", { cx: 66, cy: 426, r: 22, fill: "#ff8a3d" }),
            h("path", { d: "M56 432h20l-3-4v-6a7 7 0 0 0-14 0v6zM63 435.5a3 3 0 0 0 6 0", fill: "none", stroke: white, strokeWidth: 2.2, strokeLinejoin: "round", strokeLinecap: "round" }),
            h("text", Object.assign({ x: 100, y: 420, fill: white, fontSize: 14, fontWeight: 600 }, font), "Ringing from EchoFiles"),
            h("text", Object.assign({ x: 100, y: 440, fill: white, fillOpacity: 0.72, fontSize: 12 }, font), "Swipe to stop"))
          : h("g", { key: "nt" },
            h("rect", { x: 26, y: 392, width: 252, height: 78, rx: 26, fill: "#0b0f24", fillOpacity: 0.55 }),
            h("rect", { x: 42, y: 408, width: 18, height: 18, rx: 9, style: { fill: "var(--accent)" } }),
            h("path", { d: "M46.5 415l4.5-1.4 4.5 1.4v5.2l-4.5 1.4-4.5-1.4z", fill: "none", stroke: white, strokeWidth: 1.4, strokeLinejoin: "round" }),
            h("text", Object.assign({ x: 68, y: 421, fill: white, fillOpacity: 0.75, fontSize: 11 }, font), "EchoFiles  ·  now"),
            h("text", Object.assign({ x: 42, y: 443, fill: white, fontSize: 13, fontWeight: 600 }, font), "Connected to your laptop"),
            h("text", Object.assign({ x: 42, y: 460, fill: white, fillOpacity: 0.72, fontSize: 11.5 }, font), p.notice || "Files, photos and clipboard ready")),
        // in-display fingerprint, corner shortcuts, gesture handle
        h("g", { key: "fp", fill: "none", stroke: white, strokeOpacity: 0.8, strokeWidth: 1.5, strokeLinecap: "round" },
          h("path", { d: "M145 530a7 7 0 0 1 14 0v6M141 528a11 11 0 0 1 22 0v8M152 530v10M148 534v5M156 534v4" })),
        h("g", { key: "sc", fill: white, fillOpacity: 0.12 }, h("circle", { cx: 52, cy: 568, r: 19 }), h("circle", { cx: 252, cy: 568, r: 19 })),
        h("g", { key: "sci", fill: "none", stroke: white, strokeWidth: 1.6, strokeLinejoin: "round", strokeLinecap: "round" },
          h("path", { d: "M46 561c0 7 4 12 12 13l2-3.5-3.5-2.5-2 1.6c-2-1-3.6-2.6-4.6-4.6l1.6-2-2.5-3.5z" }),
          h("rect", { x: 243, y: 562, width: 18, height: 12, rx: 2.5 }), h("circle", { cx: 252, cy: 568, r: 3 })),
        h("rect", { key: "gb", x: 128, y: 596, width: 48, height: 3, rx: 1.5, fill: white, fillOpacity: 0.7 })]);
    return h("div", { className: cx("ef-phone", "ef-phone-" + state), style: { width: w }, role: "img", "aria-label": (p.name || "Phone") + (off ? ", not nearby" : ", " + lv + "% battery") },
      state === "ringing" ? h("span", { className: "ef-phone-waves", "aria-hidden": "true" }, h("i"), h("i"), h("i")) : null,
      h("svg", { viewBox: "0 0 304 620", width: "100%", style: { display: "block", overflow: "visible" } },
        h("defs", null, h("clipPath", { id: id + "clip" }, h("rect", { x: X, y: Y, width: W, height: H, rx: R }))),
        // volume rocker and side key, both on the right like Galaxy phones
        h("rect", { x: 300, y: 128, width: 4, height: 70, rx: 1.5, style: { fill: "var(--icon-slate)" } }),
        h("rect", { x: 300, y: 214, width: 4, height: 38, rx: 1.5, style: { fill: "var(--icon-slate)" } }),
        // frame
        h("rect", { x: 2, y: 2, width: 300, height: 616, rx: 40, style: { fill: "var(--icon-slate-deep)" } }),
        // antenna bands
        h("g", { style: { fill: "var(--icon-slate)" } },
          h("rect", { x: 2, y: 70, width: 4, height: 3 }), h("rect", { x: 298, y: 70, width: 4, height: 3 }),
          h("rect", { x: 2, y: 548, width: 4, height: 3 }), h("rect", { x: 298, y: 548, width: 4, height: 3 })),
        // glass with thin, even black border
        h("rect", { x: 8, y: 8, width: 288, height: 604, rx: 35, fill: "#020203" }),
        h("g", { clipPath: u("clip") }, screen),
        // pin-hole front camera
        h("circle", { cx: 152, cy: 31, r: 6, fill: "#000" }),
        h("circle", { cx: 152, cy: 31, r: 3, fill: "#0e1328" }),
        h("circle", { cx: 150.8, cy: 29.8, r: 1, fill: "#7684ff", fillOpacity: 0.7 })));
  }

  /** The phone in the sidebar: Connect phone, or its name, how it's reached and its battery. */
  function PhoneItem(p) {
    var s = p.state || "connected";
    if (s === "none") return h(SidebarItem, { icon: "plus", label: "Connect phone", active: p.active });
    var on = s === "connected";
    var meta = s === "pairing" ? h(StatePill, { tone: "info" }, "Pairing…") : s === "away" ? (p.seen ? "Not nearby · " + p.seen : "Not nearby") : "Wi-Fi · " + (p.network || "home");
    return h("div", { className: cx("ef-net ef-phone-item", s === "away" && "ef-net-off"), "aria-current": p.active ? "true" : undefined, role: "button", tabIndex: 0, title: on ? p.name + " · " + p.battery + "% battery" : p.name },
      h(Icon, { name: "phone", size: 16, className: "ef-phone-item-icon" }),
      h("span", { className: "ef-net-name" }, p.name || "Galaxy S24"),
      on ? h(BatteryMeter, { level: p.battery === undefined ? 72 : p.battery, charging: p.charging, label: false }) : h("span"),
      h("span", { className: "ef-net-meta" }, meta, on ? h("span", { className: "ef-phone-item-pct" }, (p.battery === undefined ? 72 : p.battery) + "%") : null));
  }

  /** One fact on the hub: label, big value, a meter and a line under it. */
  function PhoneStat(p) {
    return h("div", { className: "ef-phstat" },
      h("span", { className: "ef-phstat-label" }, h(Icon, { name: p.icon, size: 14 }), p.label),
      h("span", { className: "ef-phstat-value" }, p.value, p.unit ? h("small", null, p.unit) : null),
      p.meter !== undefined ? h("span", { className: "ef-phstat-bar" }, h("i", { style: { width: p.meter + "%", background: p.color || "var(--accent)" } })) : null,
      h("span", { className: "ef-phstat-sub" }, p.sub));
  }

  /** A hub fact as a ring gauge: what it is, the number, one line under it. */
  function Gauge(p) {
    var r = 22, C = 2 * Math.PI * r, pct = Math.max(0, Math.min(100, p.pct || 0));
    return h("div", { className: "ef-gauge" },
      h("span", { className: "ef-gauge-ring" },
        h("svg", { viewBox: "0 0 56 56", width: 56, height: 56, "aria-hidden": "true" },
          h("circle", { cx: 28, cy: 28, r: r, fill: "none", stroke: "var(--line)", strokeWidth: 5 }),
          h("circle", { className: "ef-gauge-arc", cx: 28, cy: 28, r: r, fill: "none", stroke: p.color || "var(--accent)", strokeWidth: 5, strokeLinecap: "round", strokeDasharray: C, strokeDashoffset: C * (1 - pct / 100), transform: "rotate(-90 28 28)" })),
        h(Icon, { name: p.bolt ? "bolt" : p.icon, size: 16, className: cx("ef-gauge-glyph", p.bolt && "ef-gauge-bolt") })),
      h("span", { className: "ef-gauge-text" },
        h("span", { className: "ef-gauge-label" }, p.label),
        h("span", { className: "ef-gauge-value" }, p.value, p.unit ? h("small", null, p.unit) : null),
        h("span", { className: "ef-gauge-sub" }, p.sub)));
  }

  /** Files coming from the phone: name, progress or when it landed, and where. */
  function ReceivedList(p) {
    var items = p.items || [
      { name: "IMG_20261002_1031.jpg", size: "4.1 MB", progress: 62, done: "2.5 of 4.1 MB · 18 MB/s" },
      { name: "boarding-pass.pdf", size: "212 KB", when: "2 min ago" },
      { name: "VID_20261001_2210.mp4", size: "84 MB", when: "Yesterday" }];
    return h("div", { className: "ef-recv" },
      h("div", { className: "ef-recv-head" },
        h("span", { className: "ef-group-head", style: { height: "auto" } }, p.title || "Received from phone"),
        h(Button, { size: "sm", variant: "ghost", icon: "folder" }, "Open folder")),
      items.length === 0 ? h("p", { className: "ef-muted", style: { fontSize: 12, margin: "8px 0 0" } }, "Share a file to your laptop from any app on the phone. It lands in ~/Downloads/Phone.") :
        items.map(function (f, i) {
          var going = f.progress !== undefined && f.progress < 100;
          return h("div", { key: i, className: "ef-recv-row" },
            h(FileIcon, { name: iconFor(f.name), size: 28 }),
            h("span", { className: "ef-recv-text" },
              h("span", { className: "ef-recv-name" }, f.name),
              going ? h("span", { className: "ef-progress", role: "progressbar", "aria-valuenow": f.progress }, h("i", { style: { width: f.progress + "%" } }))
                : h("span", { className: "ef-recv-sub" }, f.size + " · " + f.when)),
            going ? h("span", { className: "ef-recv-sub" }, f.done) : h(Button, { size: "sm", variant: "ghost" }, "Show"),
            going ? h(IconButton, { icon: "close", label: "Stop receiving" }) : null);
        }));
  }

  /** One feature on the hub: what it is, one live line, and where it goes. */
  function FeatureTile(p) {
    return h("button", { type: "button", className: cx("ef-feat", p.off && "ef-feat-off") },
      h("span", { className: "ef-feat-icon" }, h(Icon, { name: p.icon, size: 18 })),
      h("span", { className: "ef-feat-name" }, p.label),
      h("span", { className: "ef-feat-sub" }, p.off ? "Off · turn on in settings" : p.sub),
      p.badge && !p.off ? h("span", { className: "ef-feat-badge" }, p.badge) : null);
  }

  /** Shared clipboard on the hub: the switch and the last few things that crossed. */
  function ClipboardCard(p) {
    var items = p.items || [["from", "https://maps.app.goo.gl/x7Kd…", "1 min ago"], ["to", "ssh aditya@build-box", "6 min ago"], ["from", "OTP 482 913", "22 min ago"]];
    return h("div", { className: "ef-phcard" },
      h("div", { className: "ef-phcard-head" }, h(Icon, { name: "clipboard", size: 16 }), h("strong", null, "Shared clipboard"), h("span", { className: "ef-status-grow" }), h(Switch, { defaultChecked: p.on !== false, label: "Shared clipboard" })),
      items.map(function (it, i) {
        return h("div", { key: i, className: "ef-clip-row" },
          h(Icon, { name: it[0] === "from" ? "arrow-down" : "arrow-up", size: 14, className: "ef-clip-dir" }),
          h("span", { className: "ef-clip-text" }, it[1]),
          h("span", { className: "ef-clip-when" }, it[2]),
          h(IconButton, { icon: "copy", label: "Copy again" }));
      }),
      h("p", { className: "ef-phcard-foot" }, "Copy on one, paste on the other. Works while EchoFiles is open."));
  }

  var PHONE_DEMO = { name: "Galaxy S24", battery: 72, charging: true, network: "home", storage: [41, 128] };

  /** The phone's home: the device, its facts, Ring / Send / Get, what came in, and every feature. */
  function PhoneHub(p) {
    var ring = useState(!!p.ringing);
    var state = p.state || "connected";
    var away = state === "away";
    var d = Object.assign({}, PHONE_DEMO, p.phone || {});
    var off = p.off || {};
    return h("div", { className: "ef-hub" },
      h("div", { className: cx("ef-hub-hero", away && "ef-hub-hero-away") },
        h("div", { className: "ef-hub-stage" },
          h(PhoneDevice, { state: away ? "away" : ring[0] ? "ringing" : "connected", wallpaper: p.wallpaper, battery: d.battery, name: d.name, width: p.deviceWidth || 188 })),
        h("div", { className: "ef-hub-facts" },
          h("div", { className: "ef-hub-top" },
            away ? h(StatePill, { tone: null }, "Not nearby · last seen 2 h ago") : h(StatePill, { tone: "success" }, "Connected"),
            h(IconButton, { icon: "settings", label: "Phone settings" })),
          h("h1", { className: "ef-hub-name" }, d.name),
          h("div", { className: "ef-hub-meta" },
            h("span", null, h(Icon, { name: "wifi", size: 13 }), d.network + " Wi-Fi"),
            h("span", null, h(Icon, { name: "link", size: 13 }), "KDE Connect"),
            h("span", null, h(Icon, { name: "shield", size: 13 }), "Paired 2 Oct")),
          h("div", { className: "ef-hub-gauges" },
            h(Gauge, { icon: "battery", label: "Battery", value: d.battery, unit: "%", pct: d.battery, color: d.battery <= 20 ? "var(--warning)" : "var(--success)", sub: away ? "2 h ago" : d.charging ? "Charging" : "On battery", bolt: d.charging && !away }),
            h(Gauge, { icon: "sd-card", label: "Storage", value: d.storage[0], unit: " GB free", pct: Math.round(100 - d.storage[0] / d.storage[1] * 100), color: "var(--accent)", sub: "of " + d.storage[1] + " GB" }),
            h("button", { type: "button", className: "ef-gauge ef-newphotos" },
              h("span", { className: "ef-newphotos-stack", "aria-hidden": "true" }, [2, 1, 0].map(function (k) { return h("img", { key: k, src: photoSrc(k), alt: "" }); })),
              h("span", { className: "ef-gauge-text" },
                h("span", { className: "ef-gauge-label" }, "New photos"),
                h("span", { className: "ef-gauge-value" }, "32"),
                h("span", { className: "ef-gauge-sub" }, "Import", h(Icon, { name: "arrow-right", size: 12 }))))),
          h("div", { className: "ef-hub-dock", role: "group", "aria-label": "Phone actions" },
            h("button", { type: "button", className: cx("ef-dock-btn", ring[0] && "ef-dock-on"), disabled: away, onClick: function () { ring[1](!ring[0]); } }, h(Icon, { name: "phone-ring", size: 14 }), ring[0] ? "Stop ringing" : "Ring phone"),
            h("button", { type: "button", className: "ef-dock-btn", disabled: away }, h(Icon, { name: "upload", size: 14 }), "Send files"),
            h("button", { type: "button", className: "ef-dock-btn", disabled: away }, h(Icon, { name: "download", size: 14 }), "Get files")),
          ring[0] ? h("p", { className: "ef-hub-hint" }, "Ringing at full volume, even on silent. It stops when you tap the phone or press Stop ringing.") : null)),
      h("div", { className: "ef-hub-grid" },
        h("div", { className: "ef-hub-col" },
          h("div", { className: "ef-feats" },
            h(FeatureTile, { icon: "folder", label: "Files", sub: "Camera, Downloads, WhatsApp…" }),
            h(FeatureTile, { icon: "image", label: "Photos", sub: "2,481 photos", badge: "32 new" }),
            h(FeatureTile, { icon: "message", label: "Messages", sub: "Priya: See you at 7?", badge: "3", off: off.messages }),
            h(FeatureTile, { icon: "bell", label: "Notifications", sub: "WhatsApp, Gmail, Swiggy", badge: "5", off: off.notifications })),
          h(ReceivedList, { items: p.received })),
        h("div", { className: "ef-hub-col" }, off.clipboard ? h("div", { className: "ef-phcard" }, h(FeatureTile, { icon: "clipboard", label: "Shared clipboard", off: true })) : h(ClipboardCard, null))));
  }

  var PHONE_STEPS = ["Get the app", "Choose your phone", "Check the code", "Allow files"];
  /** Connect phone: four steps inside EchoFiles; the stock KDE Connect app is the only thing on the phone. */
  function PairPhone(p) {
    var st = useState(p.step || 0);
    var step = st[0];
    var next = function () { st[1](Math.min(3, step + 1)); };
    var body, foot;
    if (step === 0) {
      body = [
        h("p", { key: "a" }, "EchoFiles talks to the free KDE Connect app on your Android phone. Install it, open it, and keep the phone on the same Wi-Fi as this laptop."),
        h("div", { key: "b", className: "ef-pair-stores" },
          h("div", { className: "ef-pair-store" }, h(Icon, { name: "download", size: 16 }), h("span", null, h("strong", null, "Google Play"), h("small", null, "KDE Connect · KDE Community"))),
          h("div", { className: "ef-pair-store" }, h(Icon, { name: "download", size: 16 }), h("span", null, h("strong", null, "F-Droid"), h("small", null, "org.kde.kdeconnect_tp")))),
        h("p", { key: "c", className: "ef-muted" }, "No KDE apps are needed on this laptop — EchoFiles handles the connection itself.")];
      foot = [h(Button, { key: "c", variant: "ghost", kbd: ["Esc"] }, "Cancel"), h(Button, { key: "n", variant: "primary", kbd: ["Enter"], onClick: next }, "It's open on my phone")];
    } else if (step === 1) {
      var none = p.nearby === "none";
      body = [
        h("p", { key: "a" }, "Phones running KDE Connect on this network:"),
        h("div", { key: "b", className: "ef-pair-list" },
          none ? h("div", { className: "ef-pair-wait" }, h(Spinner, null), "Looking for phones…") :
            [["Galaxy S24", "Phone · 192.168.1.42"], ["Pixel Tablet", "Tablet · 192.168.1.57"]].map(function (r, i) {
              return h("button", { key: i, type: "button", className: "ef-pair-dev", onClick: next },
                h(Icon, { name: "phone", size: 18 }), h("span", { className: "ef-listrow-text" }, h("span", null, r[0]), h("span", { className: "ef-listrow-sub" }, r[1])), h("span", { className: "ef-pair-go" }, "Pair", h(Icon, { name: "chevron-right", size: 12 })));
            })),
        none ? h("div", { key: "c", className: "ef-pair-help" },
          h("strong", null, "Can't see your phone after 10 seconds?"),
          h("ol", null,
            h("li", null, "Check both are on the same Wi-Fi, and the KDE Connect app is open."),
            h("li", null, "Your firewall may be blocking it. ", h(Button, { size: "sm", icon: "shield" }, "Allow KDE Connect…"), h("span", { className: "ef-muted" }, " asks for your password once and opens ports 1714–1764.")),
            h("li", null, "Or type the phone's address in KDE Connect → Add device by IP: ", h("code", null, "192.168.1.20")))) : null];
      foot = [h(Button, { key: "c", variant: "ghost", kbd: ["Esc"] }, "Cancel"), h("span", { key: "g", className: "ef-grow" }), h(IconButton, { key: "r", icon: "refresh", label: "Look again" })];
    } else if (step === 2) {
      body = [
        h("p", { key: "a" }, "Your phone is asking to pair. Make sure it shows the same code, then tap Accept on the phone."),
        h("div", { key: "b", className: "ef-pair-code", "aria-label": "Pairing code 4F9A 2C71" }, ["4F", "9A", "2C", "71"].map(function (c, i) { return h("span", { key: i }, c); })),
        h("div", { key: "c", className: "ef-pair-wait" }, h(Spinner, null), "Waiting for Galaxy S24…")];
      foot = [h(Button, { key: "c", variant: "ghost", kbd: ["Esc"] }, "Codes don't match"), h("span", { key: "g", className: "ef-grow" }), h(Button, { key: "n", onClick: next }, "Accepted on the phone")];
    } else {
      body = [
        h("p", { key: "a" }, h("strong", { style: { color: "var(--success-ink)" } }, "Paired with Galaxy S24. "), "One switch left so EchoFiles can see the phone's files:"),
        h("ol", { key: "b", className: "ef-pair-steps" },
          h("li", null, "In KDE Connect, open ", h("strong", null, "Galaxy S24 → Plugin settings"), "."),
          h("li", null, "Turn on ", h("strong", null, "Filesystem expose"), ", and allow ", h("strong", null, "All files access"), " when Android asks.")),
        h("div", { key: "c", className: "ef-pair-check" }, h(StatePill, { tone: p.filesOk ? "success" : "warning" }, p.filesOk ? "Files available" : "Files not shared yet"), h("span", { className: "ef-muted" }, p.filesOk ? "Everything's ready." : "EchoFiles checks again on its own."))];
      foot = [h(Button, { key: "s", variant: "ghost" }, "Skip for now"), h("span", { key: "g", className: "ef-grow" }), h(Button, { key: "n", variant: "primary", icon: "phone", kbd: ["Enter"] }, "Open Galaxy S24")];
    }
    return h(Dialog, { title: "Connect your phone", icon: "phone", height: p.height || 520, footer: foot },
      h("ol", { className: "ef-stepper", "aria-label": "Steps" }, PHONE_STEPS.map(function (s, i) {
        return h("li", { key: i, className: cx(i < step && "ef-step-done", i === step && "ef-step-now"), onClick: function () { st[1](i); } },
          h("span", { className: "ef-step-n" }, i < step ? h(Icon, { name: "check", size: 11, strokeWidth: 3 }) : i + 1), s);
      })),
      body);
  }

  /** A phone asking to send files while auto-accept is off. */
  function ReceiveToast(p) {
    return h(Toast, { title: (p.from || "Galaxy S24") + " wants to send " + (p.what || "3 files · 12 MB"), icon: "download",
      actions: [h(Button, { key: "a", size: "sm", variant: "primary" }, "Accept"), h(Button, { key: "d", size: "sm", variant: "ghost" }, "Decline")] },
      "IMG_2041.jpg, boarding-pass.pdf and 1 more · saves to ~/Downloads/Phone");
  }

  var PHOTO_DAYS = [
    ["Today", [[0], [1], [2, "screenshot"], [3], [4, "portrait"], [5], [6]]],
    ["Yesterday", [[7, "video", "0:42"], [1], [2], [5, "portrait"], [0, "screenshot"]]],
    ["Monday, 28 September", [[3], [6], [4], [7, "video", "1:05"], [2], [1], [0], [5]]]];
  /** Every photo on the phone in one timeline, newest first, with Import new and Save to. */
  function PhotosPage(p) {
    var sel = useState(p.selected || ["0-1", "0-3", "1-0"]);
    var has = function (k) { return sel[0].indexOf(k) >= 0; };
    var toggle = function (k) { sel[1](has(k) ? sel[0].filter(function (x) { return x !== k; }) : sel[0].concat([k])); };
    return h("div", { className: "ef-photos" },
      h("div", { className: "ef-photos-head" },
        h("div", null, h("h1", { className: "ef-shares-title" }, "Photos"), h("p", { className: "ef-muted", style: { margin: 0 } }, "2,481 photos and videos from Galaxy S24")),
        h("span", { className: "ef-status-grow" }),
        h(SegmentedControl, { label: "Show", value: "all", options: [{ value: "all", text: "All" }, { value: "camera", text: "Camera" }, { value: "shots", text: "Screenshots" }, { value: "chat", text: "WhatsApp" }] }),
        h(Button, { variant: "primary", icon: "download" }, "Import 32 new")),
      sel[0].length ? h("div", { className: "ef-photos-bar" },
        h("strong", null, sel[0].length + " selected"), h("span", { className: "ef-muted" }, "· 11.8 MB"),
        h("span", { className: "ef-status-grow" }),
        h(Button, { size: "sm", icon: "download" }, "Save to…"), h(Button, { size: "sm", icon: "copy" }, "Copy"), h(Button, { size: "sm", variant: "ghost", onClick: function () { sel[1]([]); } }, "Clear")) : null,
      PHOTO_DAYS.map(function (day, di) {
        return h("section", { key: di, className: "ef-photos-day" },
          h("h3", { className: "ef-photos-date" }, day[0], h("span", null, day[1].length)),
          h("div", { className: "ef-photos-grid" }, day[1].map(function (ph, pi) {
            var k = di + "-" + pi;
            return h("button", { key: k, type: "button", className: cx("ef-photo", has(k) && "ef-photo-sel", ph[1] === "screenshot" && "ef-photo-shot"), "aria-pressed": String(has(k)), onClick: function () { toggle(k); } },
              h("img", { src: photoSrc(ph[0], ph[1] === "video" ? null : ph[1]), alt: "" }),
              ph[1] === "video" ? h("span", { className: "ef-photo-dur" }, h(Icon, { name: "play", size: 10 }), ph[2]) : null,
              h("span", { className: "ef-photo-check" }, h(Icon, { name: "check", size: 12, strokeWidth: 3 })));
          })));
      }));
  }

  var THREADS = [
    ["Priya", "See you at 7? I'll bring the charger", "10:31", 2],
    ["Mom", "Call me when you're free", "09:12", 1],
    ["HDFC Bank", "Your OTP is 482913. Do not share it…", "08:40"],
    ["Rahul", "Sent you the slides", "Yesterday"],
    ["Swiggy", "Your order is on the way", "Yesterday"]];
  /** Texts from the phone: conversations on the left, the chat on the right, a reply box that sends through the phone. */
  function MessagesPage(p) {
    var act = useState(0);
    return h("div", { className: "ef-msgs" },
      h("div", { className: "ef-msgs-list" },
        h("div", { className: "ef-msgs-search" }, h(SearchField, { placeholder: "Search messages" })),
        THREADS.map(function (t, i) {
          return h("button", { key: i, type: "button", className: "ef-thread", "aria-current": i === act[0] ? "true" : undefined, onClick: function () { act[1](i); } },
            h("span", { className: "ef-avatar", style: { background: "var(--icon-" + ["purple", "orange", "blue", "green", "red"][i] + ")" } }, t[0][0]),
            h("span", { className: "ef-thread-text" }, h("span", { className: "ef-thread-name" }, t[0]), h("span", { className: "ef-thread-last" }, t[1])),
            h("span", { className: "ef-thread-meta" }, h("span", null, t[2]), t[3] ? h("span", { className: "ef-thread-unread" }, t[3]) : null));
        })),
      h("div", { className: "ef-chat" },
        h("div", { className: "ef-chat-head" }, h("span", { className: "ef-avatar", style: { background: "var(--icon-purple)" } }, "P"), h("strong", null, THREADS[act[0]][0]), h("span", { className: "ef-muted" }, "+91 98xxx xx210")),
        h("div", { className: "ef-chat-body" },
          h("div", { className: "ef-chat-day" }, "Today"),
          h("div", { className: "ef-bubble" }, "Are we still on for dinner?", h("time", null, "10:24")),
          h("div", { className: "ef-bubble ef-bubble-me" }, "Yes! Booked the table for 7:30", h("time", null, "10:27")),
          h("div", { className: "ef-bubble" }, "See you at 7? I'll bring the charger", h("time", null, "10:31"))),
        h("div", { className: "ef-chat-compose" },
          h("div", { className: "ef-field", style: { flex: 1 } }, h("input", { placeholder: "Text message", "aria-label": "Text message" })),
          h(Button, { variant: "primary", icon: "send", kbd: ["Enter"] }, "Send")),
        h("p", { className: "ef-chat-foot" }, "Sends from Galaxy S24 as a normal SMS. Carrier rates apply.")));
  }

  var NOTES = [
    ["WhatsApp", "green", [["Priya", "See you at 7? I'll bring the charger", "now", true], ["Family", "Mom: sent a photo", "12 min", true]]],
    ["Gmail", "red", [["Your boarding pass", "IndiGo · 6E 2134 · BLR → DEL", "1 h", false]]],
    ["Swiggy", "orange", [["Order on the way", "Arriving in 12 minutes", "8 min", false]]]];
  /** Notifications from the phone, grouped by app: dismiss, or reply where the app allows it. */
  function NotificationsPage(p) {
    var mode = p.mode || "app";
    return h("div", { className: "ef-notes" },
      h("div", { className: "ef-photos-head" },
        h("div", null, h("h1", { className: "ef-shares-title" }, "Notifications"),
          h("p", { className: "ef-muted", style: { margin: 0 } }, mode === "desktop" ? "Shown here and as desktop pop-ups." : "Shown here only — no desktop pop-ups. Change it in phone settings.")),
        h("span", { className: "ef-status-grow" }),
        h(Button, { variant: "ghost", icon: "close" }, "Dismiss all")),
      NOTES.map(function (g, gi) {
        return h("section", { key: gi, className: "ef-note-group" },
          h("div", { className: "ef-note-app" }, h("span", { className: "ef-app-dot", style: { background: "var(--icon-" + g[1] + ")" } }, g[0][0]), g[0], h("span", { className: "ef-muted" }, "· " + g[2].length)),
          g[2].map(function (n, ni) {
            return h("div", { key: ni, className: "ef-note" },
              h("div", { className: "ef-note-text" }, h("strong", null, n[0]), h("span", null, n[1])),
              h("span", { className: "ef-note-when" }, n[2]),
              h(IconButton, { icon: "close", label: "Dismiss on the phone too" }),
              n[3] && gi === 0 && ni === 0 ? h("div", { className: "ef-note-reply" },
                h("div", { className: "ef-field", style: { flex: 1 } }, h("input", { placeholder: "Reply to Priya", "aria-label": "Reply" })),
                h(Button, { size: "sm", icon: "send" }, "Reply")) : null);
          }));
      }));
  }

  /** Settings → Phone: what this phone may do, receiving, and notifications. */
  function phoneSettings(p) {
    var bg = p && p.background;
    return [
      h(PageHead, { key: "h", title: "Phone" }, "Your Android phone over Wi-Fi, through the KDE Connect app. Features you turn off stop on both devices."),
      h(SettingsGroup, { key: "a", title: "This phone" },
        h("div", { className: "ef-setrow" },
          h("div", { className: "ef-phset-who" }, h(PhoneDevice, { width: 34, battery: 72 }),
            h("div", { className: "ef-setrow-text" }, h("span", { className: "ef-setrow-label" }, "Galaxy S24"), h("span", { className: "ef-setrow-desc" }, "Connected over Wi-Fi · paired 2 Oct 2026"))),
          h("div", { className: "ef-setrow-control" }, h(Button, { variant: "ghost", icon: "unlink" }, "Forget phone"))),
        h(SettingRow, { label: "Phone picture", description: "What the phone on the hub shows on its screen." }, h(SegmentedControl, { label: "Phone picture", value: "photo", options: [{ value: "photo", text: "Latest photo" }, { value: "aurora", text: "EchoFiles" }] }))),
      h(SettingsGroup, { key: "b", title: "Features" },
        h(SettingRow, { label: "Files and photos", description: "Browse the phone in the sidebar and copy both ways. Needs Filesystem expose on in KDE Connect." }, h(Switch, { defaultChecked: true, label: "Files and photos" })),
        h(SettingRow, { label: "Shared clipboard", description: "Copy on one, paste on the other. Works while EchoFiles is open." }, h(Switch, { defaultChecked: true, label: "Shared clipboard" })),
        h(SettingRow, { label: "Messages", description: "Read and send texts from the laptop. The phone asks for SMS permission the first time." }, h(Switch, { label: "Messages" })),
        h(SettingRow, { label: "Low battery warning", description: "A notice when the phone drops to 15%." }, h(Switch, { defaultChecked: true, label: "Low battery warning" }))),
      h(SettingsGroup, { key: "c", title: "Receiving files" },
        h(SettingRow, { label: "Accept files automatically", description: "Files you share to this laptop save straight away. Turn off to approve each one." }, h(Switch, { defaultChecked: true, label: "Accept files automatically" })),
        h(SettingRow, { label: "Save to", description: "~/Downloads/Phone" }, h(Button, { variant: "ghost", icon: "folder" }, "Change…"))),
      h(SettingsGroup, { key: "d", title: "Notifications" },
        h(SettingRow, { label: "Phone notifications", description: "Off brings nothing over. In app shows them on the phone's Notifications page only. Desktop too also pops them up like any other notification." },
          h(SegmentedControl, { label: "Phone notifications", value: "app", options: [{ value: "off", text: "Off" }, { value: "app", text: "In app" }, { value: "desktop", text: "Desktop too" }] })),
        bg ? null : h(SettingBlock, { muted: true }, "Desktop too needs Keep running in the background (General), so pop-ups arrive with the window closed."),
        h(SettingBlock, { muted: true }, "Never show notifications from:"),
        h(NameChips, { id: "napps", names: ["Instagram", "Google Photos", "Play Store"], placeholder: "App name, like Instagram", inputLabel: "App to mute", reset: false }))];
  }
  function PhoneSettings(p) {
    return h(SettingsShell, { height: p.height, page: "phone" }, phoneSettings(p));
  }

  /** The whole app with the phone hub open: sidebar Phone section, hub in the pane. */
  function PhoneWindow(p) {
    var view = p.view || "hub";
    var crumbs = [{ label: "Galaxy S24", icon: "phone" }];
    if (view !== "hub") crumbs.push({ label: { photos: "Photos", messages: "Messages", notifications: "Notifications" }[view] });
    var main = view === "photos" ? h(PhotosPage, null) : view === "messages" ? h(MessagesPage, null) : view === "notifications" ? h(NotificationsPage, null) : h(PhoneHub, { state: p.state, wallpaper: p.wallpaper });
    return h("div", { className: "ef ef-app", style: { height: p.height || 760 } },
      h(TabStrip, { tabs: [{ label: "Galaxy S24", icon: "phone" }, { label: "Downloads", icon: "download" }], active: 0 }),
      h(Toolbar, { segments: crumbs }),
      h("div", { className: "ef-app-main" },
        h(DemoSidebar, { phone: p.state || "connected", phoneView: view }),
        h("div", { className: "ef-app-center" }, h("div", { className: "ef-app-scroll" }, main))),
      h(StatusBar, view === "photos" ? { count: "2,481", selected: 3, selectedSize: "11.8 MB", volume: "Phone · KDE Connect", free: "41 GB free" } : { text: "Galaxy S24 · 72% · charging", volume: "Phone · KDE Connect", free: "41 GB free" }));
  }

  // ------------------------------------------------------------ search results
  var DEMO_HITS = [
    { name: "Cargo.toml", kind: "file", location: "~/Projects/EchoFiles_Linux", size: "1.3 KB", date: "Today 00:02" },
    { name: "Cargo.toml", kind: "file", location: "~/Projects/EchoFiles_Linux/crates/core", size: "352 B", date: "Yesterday" },
    { name: "Cargo.toml", kind: "file", location: "~/Projects/EchoFiles_Linux/crates/index", size: "460 B", date: "Yesterday" },
    { name: "Cargo.toml", kind: "file", location: "~/Downloads/t3code-0.0.39-nightly.20260902.1260/native/resource-monitor", size: "298 B", date: "2 Sep" },
    { name: "cargo", kind: "folder", location: "~/.local/share", size: "—", date: "9 Sep" }];

  /** "Everywhere" results: name, where it lives, size, date — answered from the index. */
  function SearchResults(p) {
    var hits = p.hits || DEMO_HITS;
    var st = useState(0);
    return h("div", { className: "ef-results", role: "grid", "aria-label": "Search results" },
      h("div", { className: "ef-results-row ef-results-head", role: "row" }, h("span"), h("span", null, "Name"), h("span", null, "Location"), h("span", { className: "ef-num" }, "Size"), h("span", null, "Modified")),
      h("div", { className: "ef-results-body" }, hits.map(function (f, i) {
        return h("div", { key: i, role: "row", className: "ef-results-row", "aria-selected": String(i === st[0]), onClick: function () { st[1](i); } },
          h(FileIcon, { name: iconFor(f.name, f.kind), size: 18 }),
          h("span", { className: "ef-results-name" }, f.name),
          h("span", { className: "ef-results-loc", title: f.location }, f.location),
          h("span", { className: "ef-num" }, f.size), h("span", null, f.date));
      })),
      h("div", { className: "ef-results-foot" },
        h(Kbd, { keys: ["Enter"] }), h("span", null, "open"), h(Kbd, { keys: ["Alt", "Enter"] }), h("span", null, "show in folder"),
        h("span", { className: "ef-status-grow" }),
        h(Button, { icon: "folder" }, "Show in folder")));
  }

  window.Echo = {
    Icon: Icon, FileIcon: FileIcon, Button: Button, IconButton: IconButton, SegmentedControl: SegmentedControl,
    Switch: Switch, Checkbox: Checkbox, Kbd: Kbd, TextField: TextField, SearchField: SearchField, PathBar: PathBar,
    TabStrip: TabStrip, Toolbar: Toolbar, ViewButton: ViewButton, ViewMenu: ViewMenu, Sidebar: Sidebar, SidebarSection: SidebarSection, SidebarItem: SidebarItem,
    DriveItem: DriveItem, NetworkItem: NetworkItem, ConnectDialog: ConnectDialog, SignInDialog: SignInDialog, SharesPage: SharesPage, UsageBar: UsageBar, StatePill: StatePill, FileRow: FileRow, ColumnHeader: ColumnHeader,
    FileList: FileList, Skeleton: Skeleton, FileTile: FileTile, FileGrid: FileGrid, StatusBar: StatusBar,
    EmptyState: EmptyState, DriveCard: DriveCard, Spinner: Spinner, Banner: Banner, Toast: Toast,
    TransferToast: TransferToast, Dialog: Dialog, ConflictDialog: ConflictDialog, Tooltip: Tooltip,
    ContextMenu: ContextMenu, CommandPalette: CommandPalette, PreviewPane: PreviewPane, AppWindow: AppWindow,
    DualPane: DualPane, MotionSpec: MotionSpec, SettingsGroup: SettingsGroup, PathListEditor: PathListEditor, NameChips: NameChips, SettingRow: SettingRow, SettingBlock: SettingBlock, SettingsPage: SettingsPage,
    BatteryMeter: BatteryMeter, PhoneDevice: PhoneDevice, PhoneItem: PhoneItem, PhoneStat: PhoneStat, Gauge: Gauge, ReceivedList: ReceivedList, FeatureTile: FeatureTile,
    ClipboardCard: ClipboardCard, PhoneHub: PhoneHub, PairPhone: PairPhone, ReceiveToast: ReceiveToast, PhotosPage: PhotosPage, MessagesPage: MessagesPage,
    NotificationsPage: NotificationsPage, PhoneSettings: PhoneSettings, PhoneWindow: PhoneWindow, photoSrc: photoSrc,
    SettingsShell: SettingsShell, SettingsNav: SettingsNav, IndexStatus: IndexStatus, CommandList: CommandList, SearchResults: SearchResults, SearchScope: SearchScope, iconFor: iconFor, DEMO_FILES: DEMO_FILES
  };
})();
