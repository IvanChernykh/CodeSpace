#!/usr/bin/env python3
"""Run final product polish with resilient public-site updates."""

from pathlib import Path

import finalize_product as product

ROOT = Path(__file__).resolve().parents[1]


def patch_site_resilient() -> None:
    path = ROOT / "site/index.html"
    content = path.read_text(encoding="utf-8")
    replacements = [
        (
            '<meta name="description" content="CodeSpace is a local-first semantic code graph and compact context engine for AI coding agents.">',
            '<meta name="description" content="CodeSpace is a local-first repository intelligence platform with a semantic graph, compact AI context, impact analysis, MCP, skills, tasks, memory, and a localhost dashboard.">',
        ),
        (
            '<p class="hero-lede">CodeSpace builds a live semantic graph of your repository, then returns precise, bounded context instead of dumping the whole codebase into the model.</p>',
            '<p class="hero-lede">CodeSpace builds a live semantic graph of your repository and turns it into a complete local control plane: precise AI context, impact analysis, skills, MCP servers, tasks, decisions, and a production-grade localhost dashboard.</p>',
        ),
        (
            '<div><dt>Local-first</dt><dd>No cloud required</dd></div>',
            '<div><dt>Local-first</dt><dd>Source stays on your machine</dd></div>',
        ),
        (
            '<div><dt>5 tools</dt><dd>Minimal MCP surface</dd></div>',
            '<div><dt>12 MCP tools</dt><dd>Verified agent interface</dd></div>',
        ),
        (
            '<div><dt>1 binary</dt><dd>CLI · MCP · REST</dd></div>',
            '<div><dt>1 binary</dt><dd>CLI · Dashboard · MCP · REST</dd></div>',
        ),
        (
            '<span>Semantic graph</span><i></i><span>Context compaction</span><i></i><span>Blast-radius analysis</span><i></i><span>Decision memory</span><i></i><span>MCP-native</span>',
            '<span>Semantic graph</span><i></i><span>Context compaction</span><i></i><span>Blast-radius analysis</span><i></i><span>Skills & MCP control</span><i></i><span>Local dashboard</span>',
        ),
        (
            '<h2>One engine. Four hard problems solved.</h2>',
            '<h2>One engine. A complete local control plane.</h2>',
        ),
        (
            '<article><span>MCP</span><strong>5 focused tools</strong><small>IDE and agent clients</small></article>',
            '<article><span>MCP</span><strong>12 verified tools</strong><small>IDE and agent clients</small></article>',
        ),
        (
            '<article><span>REST</span><strong>Loopback API</strong><small>Local integrations</small></article>',
            '<article><span>UI</span><strong>Local dashboard</strong><small>Graph, tasks, skills, servers</small></article>',
        ),
        (
            '<article><span>RUST</span><strong>Library crate</strong><small>Embed the engine</small></article>',
            '<article><span>API</span><strong>REST + Rust crate</strong><small>Local integrations and embedding</small></article>',
        ),
        (
            'data-copy="cargo install --git https://github.com/IvanChernykh/CodeSpace\ncse init\ncse context --query &quot;authentication returns 500&quot;"',
            'data-copy="cargo install --git https://github.com/IvanChernykh/CodeSpace\ncse init\ncse dashboard\ncse context --query &quot;authentication returns 500&quot;"',
        ),
        (
            '# Index and retrieve</span>\ncse init\ncse context --query',
            '# Index, open the control plane, and retrieve</span>\ncse init\ncse dashboard\ncse context --query',
        ),
    ]
    for old, new in replacements:
        content = content.replace(old, new)
    path.write_text(content, encoding="utf-8")
    required = [
        "complete local control plane",
        "12 MCP tools",
        "CLI · Dashboard · MCP · REST",
        "Skills & MCP control",
        "Local dashboard",
        "cse dashboard",
    ]
    missing = [item for item in required if item not in content]
    if missing:
        raise SystemExit(f"site postconditions failed: {missing}")


def main() -> None:
    product.patch_dashboard_html()
    product.patch_dashboard_typescript()
    product.patch_dashboard_css()
    patch_site_resilient()
    obsolete = ROOT / "scripts/render_dashboard.py"
    if obsolete.exists():
        obsolete.unlink()
    else:
        raise SystemExit("obsolete render_dashboard.py was already absent")
    print("final product polish applied")


if __name__ == "__main__":
    main()
