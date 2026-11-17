#!/usr/bin/env python3
"""Render local Mermaid sources; reading the exported atlas needs no network."""
from __future__ import annotations

import argparse
import hashlib
import html
import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
ATLAS = ROOT / "docs/architecture"
DIAGRAMS = ATLAS / "diagrams"
THEMES = {
    "light": {"background": "#ffffff", "primaryColor": "#e8f0fb", "primaryTextColor": "#14253d", "primaryBorderColor": "#3b6494", "lineColor": "#455b73", "secondaryColor": "#e8f4ef", "tertiaryColor": "#f7f2e8", "clusterBkg": "#f5f7fa", "clusterBorder": "#9aaabd"},
    "dark": {"background": "#101b2b", "primaryColor": "#213c59", "primaryTextColor": "#f4f8fd", "primaryBorderColor": "#8cb4df", "lineColor": "#bdcde0", "secondaryColor": "#254e46", "tertiaryColor": "#4c4335", "clusterBkg": "#16263a", "clusterBorder": "#829ab5"},
    "print": {"background": "#ffffff", "primaryColor": "#ffffff", "primaryTextColor": "#111111", "primaryBorderColor": "#222222", "lineColor": "#222222", "secondaryColor": "#eeeeee", "tertiaryColor": "#f7f7f7", "clusterBkg": "#f4f4f4", "clusterBorder": "#555555"},
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_catalog() -> dict:
    catalog = json.loads((DIAGRAMS / "catalog.json").read_text())
    ids = [d["id"] for d in catalog["diagrams"]]
    if len(set(ids)) != len(ids) or any(not re.fullmatch(r"[0-9]{2}-[a-z0-9-]+", i) for i in ids):
        raise ValueError("diagram IDs must be distinct safe filenames")
    actual = {p.stem for p in DIAGRAMS.glob("*.mmd")} | {p.stem for p in DIAGRAMS.glob("[0-9][0-9]-*.json")}
    if actual != set(ids):
        raise ValueError(f"catalog/source mismatch: {actual ^ set(ids)}")
    evidence = (ATLAS / "evidence.md").read_text()
    for d in catalog["diagrams"]:
        if d.get("source", d["id"] + ".mmd") not in {d["id"] + ".mmd", d["id"] + ".json"}:
            raise ValueError("source filenames must match their safe diagram ID")
        if not d["title"] or not d["caption"] or not d["evidence"]:
            raise ValueError(f"incomplete metadata: {d['id']}")
        for item in d["evidence"]:
            if f"| {item} |" not in evidence:
                raise ValueError(f"unknown evidence {item} in {d['id']}")
    return catalog


def catalog_markdown(catalog: dict) -> str:
    lines = ["# Diagram catalog", "", "Every diagram has editable Mermaid or SVG-layout source, a caption and evidence references.",
             "Rendered SVGs are reusable in documents and slides; PNGs are convenient for", "presentation software. Color is a reading style, not a capability/status code.", "",
             "Choose **light** for ordinary slides, **dark** for a dark presentation, or", "**print** for monochrome handouts. Evidence/status text stays the same.", "",
             "Source baseline: `" + catalog["source_baseline"] + "`. See the [evidence register](evidence.md)",
             "and [audit](audit.md) for scope and later fixes. These diagrams omit details", "deliberately; the caption says which boundary or flow each one explains.", ""]
    for d in catalog["diagrams"]:
        i = d["id"]
        lines += [f"## {i}: {d['title']}", "", f"**Scope:** {d['status']}. **Evidence:** {', '.join(d['evidence'])}.", "", d["caption"], "",
                  f"![{d['title']}](diagrams/rendered/dark/{i}.svg)", "",
                  f"[Editable source](diagrams/{d.get('source', i + '.mmd')}) · [Light SVG](diagrams/rendered/light/{i}.svg) · "
                  f"[Dark SVG](diagrams/rendered/dark/{i}.svg) · [Print SVG](diagrams/rendered/print/{i}.svg) · "
                  f"[PNG](diagrams/rendered/png/{i}.png)", ""]
    return "\n".join(lines)


def check(catalog: dict) -> None:
    expected = catalog_markdown(catalog)
    if (ATLAS / "diagrams.md").read_text() != expected:
        raise ValueError("diagram catalog Markdown is stale")
    manifest = json.loads((DIAGRAMS / "rendered/manifest.json").read_text())
    if manifest["catalog_sha256"] != digest(DIAGRAMS / "catalog.json"):
        raise ValueError("rendered metadata is stale")
    if set(manifest["diagrams"]) != {d["id"] for d in catalog["diagrams"]}:
        raise ValueError("rendered diagram set differs from source")
    for d in catalog["diagrams"]:
        i = d["id"]
        source_hash = digest(DIAGRAMS / d.get("source", f"{i}.mmd"))
        item = manifest["diagrams"][i]
        if item.get("renderer_sha256") != digest(Path(__file__)):
            raise ValueError(f"diagram was built with another renderer revision: {i}")
        expected_outputs = {f"{theme}/{i}.svg" for theme in THEMES} | {f"png/{i}.png"}
        if set(item["outputs"]) != expected_outputs:
            raise ValueError(f"missing/extra themed outputs: {i}")
        if item["source_sha256"] != source_hash:
            raise ValueError(f"stale source rendering: {i}")
        for relative, checksum in item["outputs"].items():
            path = DIAGRAMS / "rendered" / relative
            if digest(path) != checksum:
                raise ValueError(f"modified or missing render: {relative}")
            if path.suffix == ".svg":
                svg = ET.fromstring(path.read_bytes())
                if svg.attrib.get("data-source-sha256") != source_hash:
                    raise ValueError(f"wrong SVG provenance: {relative}")
                if any(el.tag.endswith("foreignObject") for el in svg.iter()):
                    raise ValueError(f"nonportable SVG text: {relative}")
                if svg.attrib.get("{http://www.w3.org/XML/1998/namespace}space") != "preserve":
                    raise ValueError(f"SVG text whitespace is not explicit: {relative}")
                if not any(el.attrib.get("data-diagram-background") == "true" for el in svg):
                    raise ValueError(f"SVG requires an explicit background shape: {relative}")
    print(f"Validated {len(catalog['diagrams'])} source diagrams and all rendered outputs.")


def scene_svg(scene: dict, colors: dict, theme: str) -> str:
    """Slide-sized boundary cards with deliberate geometry and portable SVG text."""
    e = html.escape
    dark = theme == "dark"
    ink = colors["primaryTextColor"]
    muted = "#b0c1d5" if dark else "#52657b"
    fills = {"foundation": "#254e46" if dark else "#e9f4ef", "replace": "#573930" if dark else "#fff0e9",
             "build": "#504522" if dark else "#fff6d8", "qrab": colors["primaryColor"]}
    if theme == "print":
        fills = {key: "#f5f5f5" for key in fills}
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" xml:space="preserve" viewBox="0 0 1600 900" width="1600" height="900" role="img" aria-labelledby="title desc">',
           f'<title id="title">{e(scene["title"])}</title><desc id="desc">{e(scene["subtitle"] + " " + scene["note"])}</desc>',
           f'<rect data-diagram-background="true" width="1600" height="900" fill="{colors["background"]}"/>',
           '<defs><marker id="arrow" markerWidth="10" markerHeight="8" refX="9" refY="4" orient="auto"><path d="M0 0L10 4L0 8Z" fill="' + colors["lineColor"] + '"/></marker></defs>',
           '<g font-family="DejaVu Sans, sans-serif">',
           f'<text x="60" y="71" fill="{ink}" font-size="40" font-weight="700">{e(scene["title"])}</text>',
           f'<text x="60" y="114" fill="{muted}" font-size="21">{e(scene["subtitle"])}</text>']
    for b in scene["boundaries"]:
        out += [f'<rect x="{b["x"]}" y="{b["y"]}" width="{b["w"]}" height="{b["h"]}" rx="12" fill="{colors["clusterBkg"]}" stroke="{colors["clusterBorder"]}" stroke-width="2"/>',
                f'<text x="{b["x"] + 18}" y="{b["y"] + 29}" fill="{muted}" font-size="18" font-weight="600">{e(b["title"])}</text>']
    for a in scene["arrows"]:
        points = " ".join(f"{x},{y}" for x, y in a["points"])
        out.append(f'<polyline points="{points}" fill="none" stroke="{colors["lineColor"]}" stroke-width="2.5" marker-end="url(#arrow)"/>')
        if a.get("label"):
            segments = list(zip(a["points"], a["points"][1:]))
            start, stop = max(segments, key=lambda p: abs(p[0][0] - p[1][0]))
            x, y = (start[0] + stop[0]) / 2, (start[1] + stop[1]) / 2
            out.append(f'<text x="{x}" y="{y - 9}" text-anchor="middle" fill="{muted}" font-size="15">{e(a["label"])}</text>')
    for b in scene["boxes"]:
        out.append(f'<rect x="{b["x"]}" y="{b["y"]}" width="{b["w"]}" height="{b["h"]}" rx="10" fill="{fills[b["kind"]]}" stroke="{colors["primaryBorderColor"]}" stroke-width="1.5"/>')
        if b["kicker"]:
            out.append(f'<text x="{b["x"] + 16}" y="{b["y"] + 25}" fill="{muted}" font-size="13" font-weight="700" letter-spacing="0.7">{e(b["kicker"])}</text>')
            start = b["y"] + (56 if len(b["lines"]) > 1 else 61)
        else:
            start = b["y"] + b["h"] / 2 + 7
        for j, line in enumerate(b["lines"]):
            out.append(f'<text x="{b["x"] + 16}" y="{start + 28 * j}" fill="{ink}" font-size="21">{e(line)}</text>')
    out += [f'<text x="60" y="855" fill="{muted}" font-size="17">{e(scene["note"])}</text>', '</g></svg>']
    return "".join(out)


def render(catalog: dict, mermaid_dir: Path, chrome: str, selected: set[str]) -> None:
    from playwright.sync_api import sync_playwright

    # Only reviewed local files are loaded. No server, CDN or network request
    # is needed to render this set of flow/sequence/state diagrams.
    package = json.loads((mermaid_dir.parent / "package.json").read_text())
    if package["version"] != catalog["mermaid_version"]:
        raise ValueError(f"expected Mermaid {catalog['mermaid_version']}, got {package['version']}")
    output = DIAGRAMS / "rendered"
    output.mkdir(exist_ok=True)
    manifest_path = output / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {"diagrams": {}}
    manifest.update({"mermaid_version": package["version"], "catalog_sha256": digest(DIAGRAMS / "catalog.json")})
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=chrome, headless=True)
        page = browser.new_page(viewport={"width": 1800, "height": 1400}, device_scale_factor=2)
        page.route("**/*", lambda route: route.abort())
        page.set_content("<!doctype html><html><head><meta charset='utf-8'></head><body style='margin:0'></body></html>")
        page.add_script_tag(path=str(mermaid_dir / "mermaid.min.js"))
        for d in catalog["diagrams"]:
            i = d["id"]
            if selected and i not in selected:
                continue
            source_path = DIAGRAMS / d.get("source", f"{i}.mmd")
            source = source_path.read_text()
            source_hash = digest(source_path)
            item = {"source_sha256": source_hash, "renderer_sha256": digest(Path(__file__)), "outputs": {}}
            for theme, colors in THEMES.items():
                if source_path.suffix == ".json":
                    svg = scene_svg(json.loads(source), colors, theme)
                    page.set_content(svg)
                    result = {"svg": svg, "width": 1600, "height": 900}
                else:
                    result = page.evaluate("""async ({source, id, colors}) => {
                    document.body.innerHTML = '';
                    mermaid.initialize({startOnLoad:false, securityLevel:'strict', theme:'base', htmlLabels:false,
                        deterministicIds:true, deterministicIDSeed:id, fontFamily:'DejaVu Sans, sans-serif',
                        themeVariables:{...colors, fontFamily:'DejaVu Sans, sans-serif', fontSize:'17px'},
                        flowchart:{htmlLabels:false, useMaxWidth:false, curve:'linear', padding:18, nodeSpacing:34, rankSpacing:55},
                        sequence:{useMaxWidth:false, wrap:true, actorFontSize:16, messageFontSize:15, noteFontSize:14},
                        state:{useMaxWidth:false}});
                    const {svg} = await mermaid.render('diagram-' + id, source);
                    document.body.innerHTML = svg;
                    await document.fonts.ready;
                    const el = document.querySelector('svg');
                    el.style.maxWidth = 'none';
                    el.style.background = colors.background;
                    const box = el.viewBox.baseVal;
                    el.setAttributeNS('http://www.w3.org/XML/1998/namespace', 'xml:space', 'preserve');
                    const background = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
                    // Overscan covers fractional viewBox/viewport rounding in
                    // non-browser SVG renderers without changing label layout.
                    for (const [name,value] of Object.entries({x:box.x-2,y:box.y-2,width:box.width+4,height:box.height+4,fill:colors.background,'data-diagram-background':'true'})) background.setAttribute(name,value);
                    el.insertBefore(background,el.firstChild);
                    el.setAttribute('width', Math.ceil(box.width));
                    el.setAttribute('height', Math.ceil(box.height));
                    return {svg:el.outerHTML, width:Math.ceil(box.width), height:Math.ceil(box.height)};
                    }""", {"source": source, "id": f"{theme}-{i}", "colors": colors})
                # Keep the browser serializer's SVG intact: namespace prefixes
                # introduced by ElementTree can break embedded CSS selectors.
                svg = result["svg"].replace("<svg ", f'<svg data-source-sha256="{source_hash}" ', 1)
                path = output / theme / f"{i}.svg"
                path.parent.mkdir(exist_ok=True)
                path.write_text(svg + "\n")
                item["outputs"][str(path.relative_to(output))] = digest(path)
                if theme == "light":
                    png = output / "png" / f"{i}.png"
                    png.parent.mkdir(exist_ok=True)
                    page.locator("svg").screenshot(path=str(png), animations="disabled")
                    item["outputs"][str(png.relative_to(output))] = digest(png)
                    item["width"] = result["width"]
                    item["height"] = result["height"]
            manifest["diagrams"][i] = item
            manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
            print(f"Rendered {i}", flush=True)
        browser.close()
    manifest["diagrams"] = {d["id"]: manifest["diagrams"][d["id"]] for d in catalog["diagrams"] if d["id"] in manifest["diagrams"]}
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    (ATLAS / "diagrams.md").write_text(catalog_markdown(catalog))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="validate committed source/render provenance without browser dependencies")
    parser.add_argument("--mermaid-dir", type=Path, help="dist directory from the pinned Mermaid npm package")
    parser.add_argument("--chrome", default="/usr/bin/google-chrome-stable")
    parser.add_argument("--only", action="append", default=[], help="render only a named diagram; repeatable")
    args = parser.parse_args()
    catalog = load_catalog()
    if args.check:
        check(catalog)
    else:
        if not args.mermaid_dir:
            parser.error("--mermaid-dir is required for rendering")
        selected = set(args.only)
        if selected - {d["id"] for d in catalog["diagrams"]}:
            parser.error("unknown --only diagram")
        render(catalog, args.mermaid_dir.resolve(), args.chrome, selected)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError) as exc:
        sys.exit(str(exc))
