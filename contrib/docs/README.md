# Architecture document exports

The maintained prose, deck definitions and diagrams live in
[`docs/architecture`](../../docs/architecture/README.md). Reading Markdown,
SVG, PNG, PDF or PPTX assets requires no build environment. The HTML reader
and decks use local files: no CDN, hosting account or service is required.
The ZIP preserves their relative assets for sharing.

## Validate

Python 3.11+ is sufficient for the source/link/render-provenance checks:

```sh
python3 contrib/docs/check-atlas.py
python3 contrib/docs/check-atlas.py --exports
```

Checks cover local links/headings, every workspace crate's map entry,
source/render checksums, themed output completeness and portable SVG text.
Export checks compare recorded input/output hashes. These checks do not
certify architecture claims or test a live cluster.

## Rebuild

Use a disposable Python environment and installed Chromium/Chrome. These are
documentation-only dependencies, not runtime or protocol-test requirements:

```sh
python3 -m venv /tmp/qrab-doc-tools
/tmp/qrab-doc-tools/bin/pip install -r contrib/docs/requirements.txt
python3 contrib/docs/fetch-mermaid.py /tmp/qrab-doc-mermaid
/tmp/qrab-doc-tools/bin/python contrib/docs/render-atlas.py \
  --mermaid-dir /tmp/qrab-doc-mermaid/package/dist \
  --chrome /usr/bin/google-chrome-stable
/tmp/qrab-doc-tools/bin/python contrib/docs/build-atlas.py \
  --pdf --chrome /usr/bin/google-chrome-stable
python3 contrib/docs/check-atlas.py --exports
```

The explicit Mermaid fetch verifies a pinned checksum, extracts assets and
license metadata, runs no npm lifecycle scripts and makes no QEMU fetch.
Rendering/export use local inputs. Build without `--pdf` for HTML/PPTX only;
complete PDF/bundle publication requires `--pdf`.

The build creates the reader, three HTML/PPTX slide narratives with speaker
notes, a handbook PDF and a one-diagram-per-page portfolio. The portfolio
uses figure-sized technical sheets rather than shrinking long flows onto
small slides. Simple vector-layout figures become editable PPTX shapes;
Mermaid figures use PNG for PowerPoint compatibility and retain editable
source/SVG in the corpus. HTML/PDF use vector diagrams.

The manifest records tool/browser versions, hashes and PDF page counts.
Font/browser/package changes can alter layout or bytes; the exports are
source-derived, not a promise of identical bytes across machines. Use the
recorded versions and visually review changes.

## Sources and scope

Edit prose Markdown, `decks.json`, `.mmd` or vector-layout `.json`, then
regenerate. Output SVG/PNG/PDF/PPTX/HTML files are generated. Catalog entries
carry status, caption and evidence; update them with the underlying boundary.
Unmodified upstream reference PNGs retain attribution/license/source hashes.

The shareable bundle contains the reader, decks and diagram/document assets.
Source-code and deeper subsystem-guide links still need a repository checkout;
it does not bundle the source tree or QEMU. PDF/PPTX are self-contained.

Documentation export publishes no website, sends no message, builds no runtime
image and deploys no Kubernetes resource. Durable notes belong in qrab towl.
