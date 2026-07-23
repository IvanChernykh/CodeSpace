#!/usr/bin/env python3
"""Dependency-free structural validation for the CodeSpace Pages site."""
from __future__ import annotations

import re
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]
SITE = ROOT / "site"


class PageParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.ids: set[str] = set()
        self.links: list[str] = []
        self.scripts: list[str] = []
        self.styles: list[str] = []
        self.images: list[str] = []
        self.title = False
        self.h1 = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        identifier = values.get("id")
        if identifier:
            if identifier in self.ids:
                raise AssertionError(f"duplicate id: {identifier}")
            self.ids.add(identifier)
        if tag == "a" and values.get("href"):
            self.links.append(values["href"] or "")
        if tag == "script" and values.get("src"):
            self.scripts.append(values["src"] or "")
        if tag == "link" and values.get("rel") == "stylesheet" and values.get("href"):
            self.styles.append(values["href"] or "")
        if tag in {"img", "source"} and values.get("src"):
            self.images.append(values["src"] or "")
        if tag == "title":
            self.title = True
        if tag == "h1":
            self.h1 += 1


def local_path(reference: str) -> Path | None:
    parsed = urlparse(reference)
    if parsed.scheme or reference.startswith(("mailto:", "tel:", "#")):
        return None
    clean = parsed.path.lstrip("/")
    if clean.startswith("CodeSpace/"):
        clean = clean[len("CodeSpace/"):]
    return SITE / clean


def main() -> int:
    required = [
        SITE / "index.html", SITE / "styles.css", SITE / "base.css", SITE / "components.css", SITE / "responsive.css", SITE / "app.js",
        SITE / "404.html", SITE / "assets/favicon.svg", SITE / "assets/og-card.svg",
        SITE / "manifest.webmanifest", SITE / "robots.txt", SITE / "sitemap.xml",
    ]
    missing = [str(path.relative_to(ROOT)) for path in required if not path.is_file()]
    assert not missing, f"missing site assets: {missing}"

    html = (SITE / "index.html").read_text(encoding="utf-8")
    parser = PageParser()
    parser.feed(html)
    assert parser.title, "missing title"
    assert parser.h1 == 1, f"expected one h1, found {parser.h1}"
    for expected in ["main", "capabilities", "workflow", "security", "architecture", "quickstart"]:
        assert expected in parser.ids, f"missing section id: {expected}"

    references = parser.scripts + parser.styles + parser.images + parser.links
    broken: list[str] = []
    for reference in references:
        target = local_path(reference)
        if target is not None and reference not in {"/", ""} and not target.exists():
            broken.append(reference)
    assert not broken, f"broken local references: {broken}"

    css = "\n".join((SITE / name).read_text(encoding="utf-8") for name in ["styles.css", "base.css", "components.css", "responsive.css"])
    js = (SITE / "app.js").read_text(encoding="utf-8")
    assert "@media (prefers-reduced-motion: reduce)" in css
    assert "@media (max-width: 760px)" in css
    assert "IntersectionObserver" in js
    assert "navigator.clipboard" in js
    assert not re.search(r"https?://", css), "CSS must not load external assets"
    assert not re.search(r"<script[^>]+src=[\"']https?://", html), "external scripts are forbidden"

    print(f"site validation passed: {len(parser.ids)} ids, {len(references)} references, {len(html)} HTML bytes")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AssertionError as error:
        print(f"site validation failed: {error}", file=sys.stderr)
        raise SystemExit(1)
