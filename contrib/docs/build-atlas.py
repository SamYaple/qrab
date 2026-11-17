#!/usr/bin/env python3
"""Build an offline architecture reader, slide decks and PDF/PPTX exports."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import html
from importlib.metadata import version
import json
from pathlib import Path
import re
import sys
import zipfile
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[2]
ATLAS = ROOT / "docs/architecture"
EXPORTS = ATLAS / "exports"
DIAGRAMS = ATLAS / "diagrams"
ORDER = ["README", "executive", "replacement-map", "current-state", "desired-state", "code-map", "boundaries",
         "network", "storage", "identity", "authority", "operating-model", "recovery", "continuity", "verification", "lifecycle", "security", "operations", "operator-translation", "capabilities", "transition", "cutover-story",
         "decisions", "questions", "field-guide", "glossary", "presentations", "upstream-diagrams", "audit", "evidence"]
ESC = html.escape


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def slug(text: str) -> str:
    return re.sub(r"[^\w\- ]", "", text.lower()).replace(" ", "-")


CSS = """
:root{--bg:#f5f7fa;--paper:#fff;--ink:#14253d;--muted:#52657b;--line:#cbd5e1;--accent:#245d9b}
*{box-sizing:border-box}[hidden]{display:none!important}html{scroll-behavior:smooth}body{margin:0;background:var(--bg);color:var(--ink);font-family:'DejaVu Sans',Arial,sans-serif;line-height:1.55}
a{color:var(--accent)}button,input,select{font:inherit}button,select{cursor:pointer}button:focus-visible,a:focus-visible,input:focus-visible,select:focus-visible{outline:3px solid #d77d1a;outline-offset:3px}
header.reader-head{position:sticky;top:0;z-index:5;background:var(--paper);border-bottom:1px solid var(--line);padding:14px 28px;display:flex;align-items:center;gap:18px;flex-wrap:wrap}
header strong{font-size:20px}.provenance{font-size:12px;color:var(--muted)}.layout{display:grid;grid-template-columns:270px minmax(0,1fr);max-width:1660px;margin:auto}
nav.reader-nav{padding:24px 18px;position:sticky;top:110px;height:calc(100vh - 110px);overflow:auto}nav a{display:block;padding:7px 10px;border-radius:5px;text-decoration:none;font-size:14px}nav a:hover,nav a[aria-current=true]{background:#e4edf8;color:#12395f}nav input{width:100%;padding:8px;margin-bottom:14px;border:1px solid var(--line);border-radius:6px}
main{padding:30px;min-width:0}.doc{background:var(--paper);padding:38px 44px;border:1px solid var(--line);border-radius:10px;max-width:1180px;margin-bottom:28px}.js-ready .doc{display:none}.js-ready .doc.active{display:block}
h1{font-size:32px;line-height:1.2;margin:0 0 24px}h2{font-size:24px;line-height:1.3;margin-top:38px}h3{font-size:19px;margin-top:26px}h1,h2,h3{scroll-margin-top:110px}p{max-width:82ch}table{border-collapse:collapse;width:100%;font-size:14px;margin:24px 0}th{text-align:left;background:#e9eff7}td,th{border:1px solid var(--line);padding:10px;vertical-align:top}code{font-family:'DejaVu Sans Mono',monospace;font-size:.9em}pre{padding:18px;background:#eef2f7;overflow:auto;border-radius:6px}img{max-width:100%;height:auto}.doc img{display:block;margin:22px auto;max-height:700px;object-fit:contain;background:#fff}
.gallery{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:20px}.diagram-card{border:1px solid var(--line);border-radius:8px;padding:18px;background:var(--paper)}.diagram-card h3{margin:0 0 10px}.diagram-card img{width:100%;height:240px;object-fit:contain}.diagram-card p{font-size:13px}.diagram-card .scope{font-size:12px;color:var(--muted)}.diagram-card a{font-size:13px}.gallery-controls{display:flex;gap:14px;margin:20px 0;flex-wrap:wrap}.gallery-controls input{min-width:260px;max-width:100%;padding:8px}
.cover{border-left:6px solid var(--accent);padding:20px 24px;background:#edf3fb;margin-bottom:24px}.cover p{margin:6px 0}.download-list{display:flex;gap:18px;flex-wrap:wrap;font-size:14px}.dark{--bg:#101b2b;--paper:#16263a;--ink:#edf4fc;--muted:#b0c1d5;--line:#496078;--accent:#9cc8ff}.dark th{background:#233c59}.dark pre,.dark .cover{background:#21354f}.dark nav a:hover,.dark nav a[aria-current=true]{background:#294764;color:#fff}
@media(max-width:850px){.layout{display:block}nav.reader-nav{position:static;height:auto;max-height:280px;border-bottom:1px solid var(--line)}main{padding:14px}.doc{padding:24px 18px}table{font-size:12px;display:block;overflow:auto}h1{font-size:27px}header.reader-head{position:static}}
@media print{@page{size:A4;margin:17mm}body{background:white;color:#111;font-size:10pt;line-height:1.4}header.reader-head,nav.reader-nav,.no-print{display:none!important}.layout{display:block}main{padding:0}.doc,.js-ready .doc{display:block!important;max-width:none;border:0;border-radius:0;padding:0;margin:0;break-before:page}.doc:first-child{break-before:auto}.doc img{max-height:175mm;max-width:100%;break-inside:avoid}h1{font-size:23pt}h2{font-size:16pt;break-after:avoid}h3{font-size:12pt;break-after:avoid}table{font-size:8pt;display:table}thead{display:table-header-group}tr{break-inside:avoid}td,th{padding:5px}a{color:#111}pre{white-space:pre-wrap;overflow:visible;font-size:8pt}.cover{background:#f5f5f5;border-color:#111}#doc-gallery{display:none!important}}
"""


def markdown(document: str) -> str:
    from markdown_it import MarkdownIt
    md = MarkdownIt("commonmark", {"html": False}).enable("table")
    tokens = md.parse((ATLAS / f"{document}.md").read_text())
    counts: dict[str, int] = {}
    for n, token in enumerate(tokens):
        if token.type == "heading_open":
            label = slug(tokens[n + 1].content.replace("`", ""))
            count = counts.get(label, 0)
            counts[label] = count + 1
            token.attrSet("id", f"{document}--{label}" + (f"-{count}" if count else ""))
        if token.type == "inline" and token.children:
            for child in token.children:
                if child.type != "link_open":
                    continue
                href = child.attrGet("href") or ""
                path, _, fragment = href.partition("#")
                if not path and fragment:
                    child.attrSet("href", f"#{document}--{fragment}")
                elif path.endswith(".md") and (ATLAS / unquote(path)).resolve().parent == ATLAS:
                    name = Path(path).stem
                    if name == "diagrams":
                        child.attrSet("href", "#doc-gallery")
                    elif name in ORDER:
                        child.attrSet("href", f"#{name}--{fragment}" if fragment else f"#doc-{name}")
    return md.renderer.render(tokens, md.options, {})


def reader(catalog: dict, decks: dict) -> Path:
    navigation, sections = [], []
    for name in ORDER:
        text = (ATLAS / f"{name}.md").read_text()
        title = re.sub(r"^# +", "", text.splitlines()[0])
        navigation.append(f'<a href="#doc-{name}" data-doc="{name}">{ESC(title)}</a>')
        sections.append(f'<article class="doc" id="doc-{name}" data-search="{ESC(text.lower(), quote=True)}">{markdown(name)}</article>')
    cards = []
    for d in catalog["diagrams"]:
        i = d["id"]
        source = d.get("source", i + ".mmd")
        cards.append(f'''<section class="diagram-card" data-filter="{ESC((i+' '+d['title']+' '+d['status']+' '+d['caption']).lower(), quote=True)}">
<h3>{ESC(d['title'])}</h3><div class="scope">{ESC(d['status'])} · {ESC(', '.join(d['evidence']))}</div>
<a class="diagram-image-link" href="diagrams/rendered/light/{i}.svg" target="_blank" rel="noopener"><img loading="lazy" data-diagram="{i}" src="diagrams/rendered/light/{i}.svg" alt="{ESC(d['title'], quote=True)}"></a>
<p>{ESC(d['caption'])}</p><a href="diagrams/{source}">Editable source</a> · <a href="diagrams/rendered/png/{i}.png">PNG</a></section>''')
    navigation.insert(0, '<a href="#doc-gallery" data-doc="gallery">Diagram gallery / reusable visuals</a>')
    links = " ".join(f'<a href="exports/{d["id"]}.html">{ESC(d["title"])} slides</a> · <a href="exports/{d["id"]}.pptx">PPTX</a> · <a href="exports/{d["id"]}.pdf">PDF</a>' for d in decks["decks"])
    gallery = f'''<article class="doc" id="doc-gallery"><h1>Diagram gallery</h1><p>Open a diagram for full-size vector detail. Each caption states its scope and evidence. Styles do not change capability status.</p>
<div class="gallery-controls"><input id="diagram-search" aria-label="Filter diagrams" placeholder="Filter diagrams / boundaries / evidence"><label>Style <select id="diagram-style"><option>light</option><option>dark</option><option>print</option></select></label></div><div class="gallery">{''.join(cards)}</div></article>'''
    script = """
const docs=[...document.querySelectorAll('.doc')], links=[...document.querySelectorAll('[data-doc]')];
function choose(){const id=decodeURIComponent(location.hash.slice(1));let doc=id.startsWith('doc-')?document.getElementById(id):document.getElementById(id)?.closest('.doc');doc ||= document.getElementById('doc-README');docs.forEach(x=>x.classList.toggle('active',x===doc));links.forEach(x=>x.setAttribute('aria-current',String('doc-'+x.dataset.doc===doc.id)));if(id&&!id.startsWith('doc-'))setTimeout(()=>document.getElementById(id)?.scrollIntoView(),0);else window.scrollTo(0,0);}
document.body.classList.add('js-ready');addEventListener('hashchange',choose);choose();
document.getElementById('doc-search').addEventListener('input',e=>{const q=e.target.value.toLowerCase();links.forEach(a=>{const d=document.getElementById('doc-'+a.dataset.doc);a.hidden=!!q&&!((d.dataset.search||d.textContent).toLowerCase().includes(q));});});
document.getElementById('diagram-search').addEventListener('input',e=>{const q=e.target.value.toLowerCase();document.querySelectorAll('.diagram-card').forEach(x=>x.hidden=!x.dataset.filter.includes(q));});
document.getElementById('reader-theme').onclick=()=>document.body.classList.toggle('dark');
document.getElementById('diagram-style').onchange=e=>{document.querySelectorAll('[data-diagram]').forEach(img=>{img.src='diagrams/rendered/'+e.target.value+'/'+img.dataset.diagram+'.svg';img.parentElement.href=img.src;});};
"""
    output = ATLAS / "atlas.html"
    output.write_text(f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>qrab / OpenStack architecture atlas</title><style>{CSS}</style></head><body>
<header class="reader-head"><strong>qrab / OpenStack</strong><span class="provenance">Source review through {ESC(catalog.get('reviewed_through', catalog['source_baseline']))[:12]} · {ESC(catalog['checked_date'])}</span><button id="reader-theme" type="button">Light / dark reader</button><a href="exports/handbook.pdf">Handbook PDF</a><a href="exports/diagrams.pdf">Diagram portfolio</a></header>
<div class="layout"><nav class="reader-nav" aria-label="Atlas documents"><input id="doc-search" aria-label="Search document text" placeholder="Search document text">{''.join(navigation)}</nav><main><div class="cover no-print"><strong>Keep the platform. Make VM ownership explicit.</strong><p>{len(ORDER)} document views · {len(catalog['diagrams'])} unique diagrams · {len(decks['decks'])} presentation narratives.</p><div class="download-list">{links}</div></div>{''.join(sections)}{gallery}</main></div><script>{script}</script></body></html>''')
    return output


SLIDE_CSS = """
*{box-sizing:border-box}body{margin:0;background:#27313f;font-family:'DejaVu Sans',Arial,sans-serif}.toolbar{position:fixed;bottom:12px;left:16px;z-index:10;display:flex;gap:10px;background:#fff;padding:8px 12px;border-radius:8px;font-size:13px}.toolbar button{cursor:pointer}.slide{width:1600px;height:900px;background:#fff;color:#14253d;position:relative;padding:70px 80px;overflow:hidden;display:none}.slide.active{display:block}.slide h1{font-size:48px;line-height:1.18;margin:0 0 46px;max-width:1400px}.slide p,.slide li{font-size:32px;line-height:1.45}.slide li{margin:0 0 32px;padding-left:12px}.slide ul{padding-left:30px;margin-top:70px}.slide.title{padding:180px 110px}.slide.title h1{font-size:72px;max-width:1300px;margin-bottom:50px}.slide.title p{font-size:35px;max-width:1250px}.slide .eyebrow{font-size:18px;letter-spacing:2px;color:#52657b;text-transform:uppercase;margin-bottom:26px}.slide .footer{position:absolute;left:80px;right:80px;bottom:30px;font-size:17px;color:#52657b;display:flex;justify-content:space-between;border-top:1px solid #cbd5e1;padding-top:12px}.slide.diagram{padding:0}.slide.diagram img{width:1600px;height:900px;object-fit:contain}.slide table{width:100%;border-collapse:collapse;margin-top:35px;font-size:27px;line-height:1.35}.slide td,.slide th{border:1px solid #9aaabd;padding:23px 24px;text-align:left;vertical-align:top}.slide th{background:#e8f0fb}.slide .cards{display:grid;grid-template-columns:repeat(3,1fr);gap:30px;margin-top:65px}.slide .card{border:1px solid #9aaabd;border-radius:12px;padding:28px;background:#f5f7fa;min-height:390px}.slide .card h2{font-size:31px;line-height:1.25;margin:0 0 32px}.slide .card p{font-size:27px;line-height:1.5}.slide.dark{background:#101b2b;color:#f4f8fd}.slide.dark .eyebrow,.slide.dark .footer{color:#bdcde0}.slide.dark th{background:#213c59}.slide.dark .card{background:#16263a}.slide.print{color:#111}.slide.print th,.slide.print .card{background:#f3f3f3}.notes{display:none;position:fixed;right:16px;bottom:65px;z-index:20;width:480px;max-height:60vh;overflow:auto;background:#fff6d8;color:#14253d;border:2px solid #52657b;padding:20px;font-size:16px}.notes.open{display:block}.viewport{width:1600px;transform-origin:top left}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}
@media print{@page{size:1600px 900px;margin:0}.toolbar,.notes{display:none!important}.viewport{transform:none!important}.slide{display:block!important;break-after:page;break-inside:avoid}.slide:last-child{break-after:auto}body{background:#fff}}
"""


def slide_html(deck: dict, catalog: dict) -> Path:
    diagrams = {d["id"]: d for d in catalog["diagrams"]}
    slides = []
    for index, s in enumerate(deck["slides"], 1):
        kind = s["kind"]
        evidence = "Evidence: " + ", ".join(s["evidence"])
        if kind == "diagram":
            i = s["diagram"]
            if i not in diagrams:
                raise ValueError(f"unknown slide diagram {i}")
            content = f'<img src="../diagrams/rendered/{deck["style"]}/{i}.svg" alt="{ESC(diagrams[i]["title"], quote=True)}"><h1 class="sr-only">{ESC(diagrams[i]["title"])}</h1>'
        else:
            content = f'<div class="eyebrow">{ESC(deck["title"])}</div><h1>{ESC(s["title"])}</h1>'
            if kind == "title":
                content += f'<p>{ESC(s["subtitle"])}</p>'
            elif kind == "points":
                content += '<ul>' + ''.join(f'<li>{ESC(x)}</li>' for x in s["points"]) + '</ul>'
            elif kind == "table":
                content += '<table><thead><tr>' + ''.join(f'<th>{ESC(x)}</th>' for x in s["headers"]) + '</tr></thead><tbody>' + ''.join('<tr>' + ''.join(f'<td>{ESC(x)}</td>' for x in row) + '</tr>' for row in s["rows"]) + '</tbody></table>'
            elif kind == "cards":
                content += '<div class="cards">' + ''.join(f'<section class="card"><h2>{ESC(c["title"])}</h2><p>{ESC(c["body"])}</p></section>' for c in s["cards"]) + '</div>'
            else:
                raise ValueError(f"unknown slide kind {kind}")
            content += f'<div class="footer"><span>{ESC(evidence)}</span><span>{index} / {len(deck["slides"])} · {ESC(deck["id"])}</span></div>'
        slides.append(f'<section class="slide {kind} {deck["style"]}" data-note="{ESC(s["notes"] + chr(10) + evidence, quote=True)}">{content}</section>')
    script = """
const slides=[...document.querySelectorAll('.slide')];let index=Math.max(0,Math.min(slides.length-1,Number(new URL(location).searchParams.get('slide')||1)-1));
function show(n){index=Math.max(0,Math.min(slides.length-1,n));slides.forEach((s,i)=>s.classList.toggle('active',i===index));document.getElementById('count').textContent=(index+1)+' / '+slides.length;document.getElementById('notes').textContent=slides[index].dataset.note;}
function fit(){const scale=Math.min(innerWidth/1600,(innerHeight-55)/900);document.querySelector('.viewport').style.transform='scale('+Math.max(.1,scale)+')';}
document.getElementById('prev').onclick=()=>show(index-1);document.getElementById('next').onclick=()=>show(index+1);document.getElementById('toggle-notes').onclick=()=>document.getElementById('notes').classList.toggle('open');document.getElementById('fullscreen').onclick=()=>document.documentElement.requestFullscreen?.();
addEventListener('keydown',e=>{if(['ArrowRight','PageDown',' '].includes(e.key)){e.preventDefault();show(index+1)}if(['ArrowLeft','PageUp'].includes(e.key)){e.preventDefault();show(index-1)}if(e.key.toLowerCase()==='n')document.getElementById('notes').classList.toggle('open')});addEventListener('resize',fit);show(index);fit();
"""
    path = EXPORTS / f"{deck['id']}.html"
    path.write_text(f'<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{ESC(deck["title"])}</title><style>{SLIDE_CSS}</style></head><body><main class="viewport">{"".join(slides)}</main><div class="toolbar"><button id="prev" type="button">Previous</button><span id="count"></span><button id="next" type="button">Next</button><button id="toggle-notes" type="button">Notes (N)</button><button id="fullscreen" type="button">Fullscreen</button><a href="../atlas.html">Atlas</a></div><aside class="notes" id="notes" aria-label="Speaker notes"></aside><script>{script}</script></body></html>')
    return path


def pptx(deck: dict, catalog: dict) -> Path:
    from pptx import Presentation
    from pptx.dml.color import RGBColor
    from pptx.enum.shapes import MSO_AUTO_SHAPE_TYPE, MSO_CONNECTOR
    from pptx.enum.text import MSO_AUTO_SIZE
    from pptx.oxml.xmlchemy import OxmlElement
    from pptx.util import Inches, Pt
    prs = Presentation()
    prs.slide_width, prs.slide_height = Inches(13.333333), Inches(7.5)
    prs.core_properties.title = deck["title"]
    prs.core_properties.subject = "qrab / OpenStack architecture comparison"
    prs.core_properties.author = "qrab"
    prs.core_properties.created = prs.core_properties.modified = datetime(2026, 11, 17, tzinfo=timezone.utc)
    dark = deck["style"] == "dark"
    colors = {"bg": "101b2b" if dark else "ffffff", "ink": "f4f8fd" if dark else "14253d", "muted": "bdcde0" if dark else "52657b", "line": "829ab5" if dark else "9aaabd", "panel": "16263a" if dark else "f5f7fa"}
    def text(slide, value, x, y, w, h, size=24, color=None, bold=False):
        shape = slide.shapes.add_textbox(Inches(x), Inches(y), Inches(w), Inches(h))
        tf = shape.text_frame
        tf.clear(); tf.word_wrap = True; tf.auto_size = MSO_AUTO_SIZE.NONE
        tf.margin_left = tf.margin_right = tf.margin_top = tf.margin_bottom = 0
        for n, line in enumerate(value.split('\n')):
            p = tf.paragraphs[0] if n == 0 else tf.add_paragraph()
            p.text = line; p.font.name = "DejaVu Sans"; p.font.size = Pt(size); p.font.bold = bold
            p.font.color.rgb = RGBColor.from_string(color or colors["ink"])
            p.space_after = Pt(7)
        return shape
    def rect(slide, x, y, w, h, fill=None):
        shape = slide.shapes.add_shape(MSO_AUTO_SHAPE_TYPE.ROUNDED_RECTANGLE, Inches(x), Inches(y), Inches(w), Inches(h))
        shape.fill.solid(); shape.fill.fore_color.rgb = RGBColor.from_string(fill or colors["panel"])
        shape.line.color.rgb = RGBColor.from_string(colors["line"])
        return shape
    diagrams = {d["id"]: d for d in catalog["diagrams"]}
    for index, s in enumerate(deck["slides"], 1):
        slide = prs.slides.add_slide(prs.slide_layouts[6])
        slide.background.fill.solid(); slide.background.fill.fore_color.rgb = RGBColor.from_string(colors["bg"])
        kind = s["kind"]
        if kind == "diagram":
            d = diagrams[s["diagram"]]
            source = DIAGRAMS / d.get("source", d["id"] + ".mmd")
            if source.suffix == ".json":
                scene = json.loads(source.read_text())
                fills = {"foundation": "254e46" if dark else "e9f4ef", "replace": "573930" if dark else "fff0e9", "build": "504522" if dark else "fff6d8", "qrab": "213c59" if dark else "e8f0fb"}
                if deck["style"] == "print": fills = {k:"f5f5f5" for k in fills}
                text(slide, scene["title"], .5, .27, 12.3, .5, 24, bold=True)
                text(slide, scene["subtitle"], .5, .78, 12.3, .45, 12.6, colors["muted"])
                for b in scene["boundaries"]:
                    rect(slide,b["x"]/120,b["y"]/120,b["w"]/120,b["h"]/120)
                    text(slide,b["title"],(b["x"]+18)/120,(b["y"]+9)/120,(b["w"]-36)/120,.3,10.8,colors["muted"],True)
                for a in scene["arrows"]:
                    segments=list(zip(a["points"],a["points"][1:]))
                    for segment,(begin, end) in enumerate(segments):
                        line=slide.shapes.add_connector(MSO_CONNECTOR.STRAIGHT, Inches(begin[0]/120), Inches(begin[1]/120), Inches(end[0]/120), Inches(end[1]/120))
                        line.line.color.rgb=RGBColor.from_string(colors["muted"]); line.line.width=Pt(1.5)
                        if segment==len(segments)-1:
                            arrow=OxmlElement('a:tailEnd');arrow.set('type','triangle')
                            line._element.spPr.get_or_add_ln().append(arrow)
                    if a.get("label"):
                        begin,end=max(zip(a["points"],a["points"][1:]),key=lambda p:abs(p[0][0]-p[1][0]))
                        x,y=(begin[0]+end[0])/240,(begin[1]+end[1])/240
                        text(slide,a["label"],x-1.8,y-.24,3.6,.3,9,colors["muted"])
                for b in scene["boxes"]:
                    rect(slide,b["x"]/120,b["y"]/120,b["w"]/120,b["h"]/120,fills[b["kind"]])
                    if b["kicker"]:
                        text(slide,b["kicker"],(b["x"]+16)/120,(b["y"]+10)/120,(b["w"]-32)/120,.23,7.8,colors["muted"],True)
                        y=(b["y"]+(34 if len(b["lines"])>1 else 39))/120
                    else: y=(b["y"]+b["h"]/2-14)/120
                    text(slide,'\n'.join(b["lines"]),(b["x"]+16)/120,y,(b["w"]-32)/120,(b["h"]-20)/120,12.6)
                text(slide,scene["note"],.5,7.02,12.3,.32,10.2,colors["muted"])
            else:
                from PIL import Image
                path=DIAGRAMS/"rendered/png"/(d["id"]+".png")
                width,height=Image.open(path).size
                scale=min(12.3/width,6.5/height)
                w,h=width*scale,height*scale
                picture=slide.shapes.add_picture(str(path), Inches((13.333333-w)/2), Inches((7.5-h)/2), width=Inches(w), height=Inches(h))
                picture._element.xpath('.//p:cNvPr')[0].set('descr',d["caption"])
        else:
            text(slide,deck["title"].upper(),.67,.4,12,.35,11,colors["muted"])
            if kind == "title":
                text(slide,s["title"],.9,1.5,11.5,2.2,43,bold=True)
                text(slide,s["subtitle"],.9,4.0,11.5,1.3,23)
            else:
                text(slide,s["title"],.67,.9,12,1.1,29,bold=True)
                if kind == "points":
                    for n,item in enumerate(s["points"]): text(slide,'•  '+item,.85,2.2+n*1.25,11.6,1.08,22)
                elif kind == "cards":
                    for n,c in enumerate(s["cards"]):
                        x=.67+n*4.05;rect(slide,x,2.35,3.8,3.65)
                        text(slide,c["title"],x+.23,2.6,3.35,1.15,21,bold=True)
                        text(slide,c["body"],x+.23,3.85,3.35,1.95,18)
                elif kind == "table":
                    rows=[s["headers"]]+s["rows"]
                    shape=slide.shapes.add_table(len(rows),len(rows[0]),Inches(.67),Inches(2.05),Inches(12),Inches(4.4))
                    table=shape.table
                    for n,row in enumerate(rows):
                        for col,value in enumerate(row):
                            cell=table.cell(n,col);cell.text=value;cell.fill.solid();cell.fill.fore_color.rgb=RGBColor.from_string(colors["panel"])
                            for p in cell.text_frame.paragraphs:
                                p.font.name='DejaVu Sans';p.font.size=Pt(17 if len(rows)>5 else 19);p.font.bold=n==0;p.font.color.rgb=RGBColor.from_string(colors["ink"])
                else: raise ValueError(kind)
            text(slide,'Evidence: '+', '.join(s["evidence"]),.67,7.05,10,.25,9,colors["muted"])
            text(slide,f'{index} / {len(deck["slides"])}',11.5,7.05,1.2,.25,9,colors["muted"])
        slide.notes_slide.notes_text_frame.text=s["notes"]+'\n\nEvidence: '+', '.join(s["evidence"])+f'\nSource review: {catalog.get("reviewed_through", catalog["source_baseline"])}\nSee docs/architecture/evidence.md and audit.md.'
    output=EXPORTS/f'{deck["id"]}.pptx'
    prs.save(output)
    return output


def portfolio(catalog: dict) -> Path:
    manifest=json.loads((DIAGRAMS/'rendered/manifest.json').read_text())
    styles, pages=[],[]
    for n,d in enumerate(catalog['diagrams']):
        item=manifest['diagrams'][d['id']]
        w=max(1000,item['width']+80);h=item['height']+320
        styles.append(f'@page figure{n}{{size:{w}px {h}px;margin:0}} #figure{n}{{page:figure{n};width:{w}px;height:{h}px}}')
        pages.append(f'<section id="figure{n}"><h1>{ESC(d["title"])}</h1><p>{ESC(d["status"])} · Evidence {ESC(", ".join(d["evidence"]))}</p><img src="../diagrams/rendered/print/{d["id"]}.svg" width="{item["width"]}" height="{item["height"]}" alt="{ESC(d["title"],quote=True)}"><p>{ESC(d["caption"])}</p></section>')
    path=EXPORTS/'diagrams.html'
    path.write_text('<!doctype html><html lang="en"><meta charset="utf-8"><title>qrab diagram portfolio</title><style>*{box-sizing:border-box}body{margin:0;font-family:DejaVu Sans,Arial,sans-serif;color:#111;line-height:1.35}section{padding:30px 40px;break-after:page;break-inside:avoid}section:last-child{break-after:auto}h1{font-size:26px;margin:0 0 10px}p{font-size:16px;max-width:1400px;margin:10px 0}img{display:block;max-width:100%;object-fit:contain}'+''.join(styles)+'</style>'+''.join(pages)+'</html>')
    return path


def pdfs(reader_path: Path, slide_paths: list[Path], portfolio_path: Path, chrome: str) -> dict:
    from playwright.sync_api import sync_playwright
    from pypdf import PdfReader
    outputs={};checks={}
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path=chrome,headless=True)
        page=browser.new_page(viewport={'width':1600,'height':1000})
        page.route(re.compile(r'^https?://'),lambda route:route.abort())
        for path in [reader_path,*slide_paths,portfolio_path]:
            errors=[]
            page.on('pageerror',lambda error:errors.append(str(error)))
            page.goto(path.as_uri(),wait_until='load');page.evaluate('document.fonts.ready')
            page.locator('img').evaluate_all('async xs=>{await Promise.all(xs.map(async x=>{x.loading="eager";try{await x.decode()}catch{}}))}')
            if errors: raise ValueError(f'{path.name}: browser errors: {errors}')
            broken=page.locator('img').evaluate_all('(xs)=>xs.filter(x=>!x.complete||x.naturalWidth===0).map(x=>x.src)')
            if broken: raise ValueError(f'{path.name}: broken images: {broken}')
            if path in slide_paths:
                page.emulate_media(media='print')
                overflow=page.locator('.slide').evaluate_all('(xs)=>xs.flatMap((s,i)=>[...s.children].filter(x=>!x.classList.contains("footer")&&!x.classList.contains("sr-only")&&x.getBoundingClientRect().bottom>s.getBoundingClientRect().bottom-42).map(x=>({slide:i+1,tag:x.tagName,bottom:x.getBoundingClientRect().bottom-s.getBoundingClientRect().top})))')
                # Full-artboard diagrams deliberately reach their slide edge.
                overflow=[x for x in overflow if x['tag']!='IMG']
                if overflow: raise ValueError(f'{path.name}: slide content overflow: {overflow}')
            out=EXPORTS/('handbook.pdf' if path==reader_path else path.stem+'.pdf')
            page.pdf(path=str(out),prefer_css_page_size=True,print_background=True)
            outputs[str(out.relative_to(ROOT))]=sha(out)
            checks[out.name]={'pages':len(PdfReader(out).pages),'bytes':out.stat().st_size}
            page.emulate_media(media='screen')
        checks['browser']=browser.version
        browser.close()
    return {'outputs':outputs,'checks':checks}


def bundle(catalog: dict, manifest: dict) -> Path:
    paths=set(ATLAS.glob('*.md')) | {ATLAS/'atlas.html', ATLAS/'decks.json'}
    paths.update(p for p in DIAGRAMS.rglob('*') if p.is_file())
    paths.update(p for p in (ATLAS/'upstream').rglob('*') if p.is_file())
    paths.update(ROOT/relative for relative in manifest['outputs'])
    files={str(p.relative_to(ATLAS)):p.read_bytes() for p in sorted(paths)}
    files['READ-ME-FIRST.txt']=(
        'qrab / OpenStack architecture presentation bundle\n\n'
        'Open atlas.html for the document library and diagram gallery.\n'
        'Open the audience-specific HTML decks in exports/ for slides.\n'
        'PDF and PPTX are self-contained; HTML uses the relative assets in this bundle.\n'
        'Source-code and deeper subsystem-guide links need the repository checkout.\n'
        'This bundle does not contain source code or QEMU.\n'
        'Original OpenStack reference PNG attribution/license is in upstream-diagrams.md.\n'
        'The supplied OpenStack cluster and the qrab cluster are distinct.\n'
        f'Source review through {catalog.get("reviewed_through",catalog["source_baseline"])}.\n'
        'Source support and dated live deployment evidence are different.\n'
        'contents.json records the SHA-256 of every packaged asset.\n'
    ).encode()
    inventory={name:hashlib.sha256(data).hexdigest() for name,data in files.items()}
    files['contents.json']=(json.dumps(inventory,indent=2,sort_keys=True)+'\n').encode()
    output=EXPORTS/'qrab-architecture.zip'
    with zipfile.ZipFile(output,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as archive:
        for name,data in sorted(files.items()):
            entry=zipfile.ZipInfo('qrab-architecture/'+name,date_time=(2026,11,17,0,0,0))
            entry.compress_type=zipfile.ZIP_DEFLATED;entry.create_system=3;entry.external_attr=0o100644<<16
            archive.writestr(entry,data)
    with zipfile.ZipFile(output) as archive:
        if archive.testzip() is not None:raise ValueError('bundle CRC validation failed')
        for name,expected in inventory.items():
            if hashlib.sha256(archive.read('qrab-architecture/'+name)).hexdigest()!=expected:
                raise ValueError(f'bundle content mismatch: {name}')
    return output


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pdf',action='store_true',help='also render PDFs with the installed Chrome')
    parser.add_argument('--chrome',default='/usr/bin/google-chrome-stable')
    args=parser.parse_args()
    EXPORTS.mkdir(exist_ok=True)
    catalog=json.loads((DIAGRAMS/'catalog.json').read_text())
    decks=json.loads((ATLAS/'decks.json').read_text())
    if set(ORDER)!=({p.stem for p in ATLAS.glob('*.md')}-{'diagrams'}):
        raise ValueError('reader document inventory is stale')
    outputs=[]
    reader_path=reader(catalog,decks);outputs.append(reader_path)
    slide_paths=[]
    for deck in decks['decks']:
        slide_paths.append(slide_html(deck,catalog));outputs.extend([slide_paths[-1],pptx(deck,catalog)])
    portfolio_path=portfolio(catalog);outputs.append(portfolio_path)
    manifest={'reviewed_through':catalog.get('reviewed_through',catalog['source_baseline']),
              'tools':{x:version(x) for x in ['markdown-it-py','python-pptx','playwright','Pillow','pypdf']},
              'inputs':{},'outputs':{str(p.relative_to(ROOT)):sha(p) for p in outputs},'checks':{}}
    inputs=[ATLAS/f'{x}.md' for x in ORDER]+[ATLAS/'decks.json',DIAGRAMS/'catalog.json',DIAGRAMS/'rendered/manifest.json',Path(__file__),ROOT/'contrib/docs/requirements.txt']
    inputs.extend(p for p in (ATLAS/'upstream').rglob('*') if p.is_file())
    for path in inputs:manifest['inputs'][str(path.relative_to(ROOT))]=sha(path)
    if args.pdf:
        result=pdfs(reader_path,slide_paths,portfolio_path,args.chrome)
        manifest['outputs'].update(result['outputs']);manifest['checks']=result['checks']
        for deck in decks['decks']:
            if manifest['checks'][deck['id']+'.pdf']['pages']!=len(deck['slides']):
                raise ValueError(f'wrong PDF page count for {deck["id"]}')
        if manifest['checks']['diagrams.pdf']['pages']!=len(catalog['diagrams']):
            raise ValueError('diagram portfolio must have exactly one page per diagram')
        package=bundle(catalog,manifest)
        manifest['outputs'][str(package.relative_to(ROOT))]=sha(package)
    (EXPORTS/'manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
    print(json.dumps({'documents':len(ORDER),'diagrams':len(catalog['diagrams']),'slides':{d['id']:len(d['slides']) for d in decks['decks']},'checks':manifest['checks']},indent=2))


if __name__=='__main__':
    try:main()
    except (ValueError,OSError,KeyError) as error:sys.exit(str(error))
