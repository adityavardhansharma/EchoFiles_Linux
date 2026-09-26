#!/usr/bin/env python3
"""Local preview of the design system: design/harness.html#<Component> (theme switcher top-right)."""
import json, pathlib, re
D = pathlib.Path(__file__).resolve().parent
t = json.load(open(D / "tokens.json"))
themes = [x["id"] for x in t["color"]["themes"]]
val = lambda tok, th: tok["value"] if isinstance(tok["value"], str) else tok["value"].get(th, tok["value"][themes[0]])
css = []
for th in themes:
    sel = ":root" if th == themes[0] else f'[data-theme="{th}"]'
    css.append(sel + "{" + "".join(f'--{c["name"]}:{val(c, th)};' for c in t["color"]["tokens"] + t["shadow"]["tokens"]) + "}")
other = [f'--{c["name"]}:{c["value"]};' for fam in ("spacing", "radius", "size", "duration", "easing", "zIndex", "opacity") for c in t[fam]["tokens"]]
css.append(":root{" + "".join(other) + "--font-mono:" + t["type"]["families"]["mono"] + ";}")
comp = D / "system/project/components"
previews = {p.parent.name: p.read_text() for p in comp.glob("*/preview.html")}
apps = {}
for name, src in previews.items():
    m = re.search(r"function App\(\)\{(.*)\}\nReactDOM", src, re.S)
    if m: apps[name] = m.group(1)
fns = "".join(f"APPS[{json.dumps(n)}]=function(){{{b}}};\n" for n, b in apps.items())
opts = "".join(f'<option value="{th}">{th}</option>' for th in themes)
html = f'''<!doctype html><html data-theme="{themes[0]}"><head><meta charset="utf-8"><title>EchoFiles harness</title><style>{"".join(css)}</style>
<style>{(comp / "bundle.css").read_text()}</style>
<script src="https://cdnjs.cloudflare.com/ajax/libs/react/18.3.1/umd/react.production.min.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/react-dom/18.3.1/umd/react-dom.production.min.js"></script>
<script>{(comp / "bundle.js").read_text()}</script></head>
<body><select id="th" style="position:fixed;right:8px;top:8px;z-index:99" onchange="document.documentElement.dataset.theme=this.value">{opts}</select>
<div id="root"></div>
<script>var E=window.Echo,h=React.createElement,APPS={{}};
{fns}
function go(){{var n=location.hash.slice(1)||"AppWindow";ReactDOM.render(h(APPS[n]||APPS.AppWindow),document.getElementById("root"));}}
window.onhashchange=go;go();</script></body></html>'''
(D / "harness.html").write_text(html)
print("harness:", len(apps), "components")
