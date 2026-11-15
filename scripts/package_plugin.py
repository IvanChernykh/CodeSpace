#!/usr/bin/env python3
"""Build and inspect a local or HTTPS CodeSpace plugin without bundling secrets."""
from __future__ import annotations
import argparse
import json
import shutil
import tempfile
import zipfile
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]

def package(output: Path, remote_url: str | None) -> None:
    source = ROOT / "plugins/codespace"
    if remote_url:
        uri = urlsplit(remote_url)
        if uri.scheme != "https" or not uri.netloc or uri.username or uri.password or uri.query or uri.fragment or not uri.path.endswith('/mcp'):
            raise ValueError("Use the verified HTTPS /mcp endpoint without credentials, query or fragment")
    with tempfile.TemporaryDirectory(prefix="codespace-plugin-") as temporary:
        plugin = Path(temporary) / 'codespace'
        shutil.copytree(source, plugin, ignore=shutil.ignore_patterns('server.mjs','widget.html','licenses.json'))
        if remote_url:
            # A remote package must contain no local process/hook dependency, or
            # clients will correctly classify it as desktop-only.
            shutil.rmtree(plugin / 'server')
            shutil.rmtree(plugin / 'scripts')
            shutil.rmtree(plugin / '.codex-plugin')
            (plugin / '.mcp.json').unlink()
            mcp = {"$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json", "mcpServers":{"codespace":{
                "type":"streamable-http", "url":remote_url, "extensions":{"com.openai":{"auth":{"type":"api_key","headerScheme":"bearer"}}}}}}
            (plugin / 'mcp.json').write_text(json.dumps(mcp, indent=2)+'\n', encoding='utf8')
            manifest = json.loads((plugin / 'plugin.json').read_text(encoding='utf8'))
            manifest['extensions']['com.openai']['onboardingSkill'] = './skills/repository-context/SKILL.md'
            (plugin/'plugin.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf8')
            shutil.rmtree(plugin/'skills/setup')
        else:
            for name in ('server.mjs','widget.html'):
                built = ROOT/'integrations/chatgpt/dist'/name
                if not built.is_file():
                    raise ValueError('Run npm ci && npm run build in integrations/chatgpt first')
                shutil.copyfile(built,plugin/'server'/name)
            # Also populate the local authoring directory for marketplace installs.
            for name in ('server.mjs','widget.html'):
                shutil.copyfile(plugin/'server'/name,source/'server'/name)
            license_files = {}
            for file in (ROOT/'integrations/chatgpt/node_modules').rglob('LICENSE*'):
                if file.is_file() and file.stat().st_size < 100_000:
                    license_files[file.relative_to(ROOT/'integrations/chatgpt/node_modules').as_posix()] = file.read_text(encoding='utf8',errors='replace')
            (plugin/'server/licenses.json').write_text(json.dumps(license_files,indent=2)+'\n',encoding='utf8')
            shutil.copyfile(plugin/'server/licenses.json',source/'server/licenses.json')
        manifest = json.loads((plugin/'plugin.json').read_text(encoding='utf8'))
        assert manifest['version'] == '3.0.0'
        assert len(manifest['extensions']['com.openai']['interface']['shortDescription']) <= 30
        for file in plugin.rglob('*'):
            if file.is_symlink(): raise ValueError('Plugin cannot contain symlinks')
            if file.suffix == '.json': json.loads(file.read_text(encoding='utf8'))
            if file.name.endswith('.local.json') or file.name.startswith('.env'): raise ValueError('Local configuration cannot be packaged')
        output.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED) as archive:
            for file in sorted(plugin.rglob('*')):
                if file.is_file(): archive.write(file, 'codespace/'+file.relative_to(plugin).as_posix())
        with zipfile.ZipFile(output) as archive:
            assert archive.testzip() is None
            files=archive.namelist()
            assert 'codespace/plugin.json' in files and 'codespace/mcp.json' in files
            assert not any('node_modules/' in file for file in files)
        print(f"Plugin verified: {output} ({len(files)} files, {'remote HTTPS' if remote_url else 'local desktop'})")

if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=ROOT/'artifacts/codespace-plugin-3.0.0.zip')
    parser.add_argument('--remote-url')
    arguments=parser.parse_args()
    package(arguments.output.resolve(), arguments.remote_url)
