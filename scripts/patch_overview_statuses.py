#!/usr/bin/env python3
"""Decouple overview subsystem probes and provide truthful initial states."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: Path, old: str, new: str) -> None:
    content = path.read_text(encoding="utf-8")
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"expected one match in {path}, found {count}")
    path.write_text(content.replace(old, new, 1), encoding="utf-8")


def patch_html() -> None:
    path = ROOT / "src/dashboard.rs"
    replacements = {
        '<strong id="aiRuntimeStatus">Checking…</strong>': '<strong id="aiRuntimeStatus">Local Ollama</strong>',
        '<strong id="skillsRuntimeStatus">Checking…</strong>': '<strong id="skillsRuntimeStatus">Built-in registry</strong>',
        '<strong id="mcpRuntimeStatus">Checking…</strong>': '<strong id="mcpRuntimeStatus">No servers</strong>',
        '<strong id="githubRuntimeStatus">Checking…</strong>': '<strong id="githubRuntimeStatus">Optional</strong>',
        '<strong id="aiHealthText">Checking…</strong>': '<strong id="aiHealthText">Local Ollama</strong>',
        '<strong id="skillsHealthText">Checking…</strong>': '<strong id="skillsHealthText">Built-in registry</strong>',
        '<strong id="mcpHealthText">Checking…</strong>': '<strong id="mcpHealthText">No servers</strong>',
        '<strong id="githubHealthText">Checking…</strong>': '<strong id="githubHealthText">Optional</strong>',
    }
    content = path.read_text(encoding="utf-8")
    for old, new in replacements.items():
        if content.count(old) != 1:
            raise SystemExit(f"expected one HTML status match: {old}")
        content = content.replace(old, new, 1)
    path.write_text(content, encoding="utf-8")


def patch_typescript() -> None:
    path = ROOT / "dashboard/src/main.ts"
    old = '''  private async loadOverviewSubsystems(): Promise<void> {
    const [skillsResult, mcpResult, settingsResult, githubResult] = await Promise.allSettled([
      this.api.skills(),
      this.api.mcp(),
      this.api.settings(),
      this.api.githubStatus(),
    ]);
    const setStatus = (primaryId: string, healthId: string, label: string, tone: "success" | "warning" | "neutral"): void => {
      $(primaryId).textContent = label;
      $(healthId).textContent = label;
      const healthRow = $(healthId).parentElement;
      if (tone === "neutral") healthRow?.removeAttribute("data-tone");
      else healthRow?.setAttribute("data-tone", tone);
    };

    if (skillsResult.status === "fulfilled") {
      const enabled = skillsResult.value.skills.filter((skill) => skill.enabled).length;
      const total = skillsResult.value.skills.length;
      setStatus("#skillsRuntimeStatus", "#skillsHealthText", `${enabled}/${total} enabled`, enabled > 0 ? "success" : "warning");
    } else {
      setStatus("#skillsRuntimeStatus", "#skillsHealthText", "Unavailable", "warning");
    }

    if (mcpResult.status === "fulfilled") {
      const running = mcpResult.value.servers.filter((server) => server.status.toLowerCase() === "running").length;
      const total = mcpResult.value.servers.length;
      const label = total === 0 ? "No servers" : `${running}/${total} running`;
      setStatus("#mcpRuntimeStatus", "#mcpHealthText", label, running > 0 ? "success" : "neutral");
    } else {
      setStatus("#mcpRuntimeStatus", "#mcpHealthText", "Unavailable", "warning");
    }

    if (settingsResult.status === "fulfilled") {
      const effective = settingsResult.value.effective;
      const model = text(effective["ollama_model"] ?? effective["ai.model"] ?? effective["model"]);
      const label = model || "Local Ollama";
      setStatus("#aiRuntimeStatus", "#aiHealthText", label, model ? "success" : "neutral");
    } else {
      setStatus("#aiRuntimeStatus", "#aiHealthText", "Not configured", "warning");
    }

    if (githubResult.status === "fulfilled") {
      const status = githubResult.value;
      const identity = text(status["username"] ?? status["login"] ?? status["user"]);
      const connected = Boolean(status["connected"] ?? status["authenticated"] ?? identity);
      setStatus("#githubRuntimeStatus", "#githubHealthText", connected ? identity || "Connected" : "Optional", connected ? "success" : "neutral");
    } else {
      setStatus("#githubRuntimeStatus", "#githubHealthText", "Optional", "neutral");
    }
  }'''
    new = '''  private loadOverviewSubsystems(): void {
    const setStatus = (primaryId: string, healthId: string, label: string, tone: "success" | "warning" | "neutral"): void => {
      $(primaryId).textContent = label;
      $(healthId).textContent = label;
      const healthRow = $(healthId).parentElement;
      if (tone === "neutral") healthRow?.removeAttribute("data-tone");
      else healthRow?.setAttribute("data-tone", tone);
    };

    void this.api.skills().then((response) => {
      const enabled = response.skills.filter((skill) => skill.enabled).length;
      const total = response.skills.length;
      setStatus("#skillsRuntimeStatus", "#skillsHealthText", `${enabled}/${total} enabled`, enabled > 0 ? "success" : "warning");
    }).catch(() => setStatus("#skillsRuntimeStatus", "#skillsHealthText", "Unavailable", "warning"));

    void this.api.mcp().then((response) => {
      const running = response.servers.filter((server) => server.status.toLowerCase() === "running").length;
      const total = response.servers.length;
      const label = total === 0 ? "No servers" : `${running}/${total} running`;
      setStatus("#mcpRuntimeStatus", "#mcpHealthText", label, running > 0 ? "success" : "neutral");
    }).catch(() => setStatus("#mcpRuntimeStatus", "#mcpHealthText", "Unavailable", "warning"));

    void this.api.settings().then((response) => {
      const effective = response.effective;
      const model = text(effective["ollama_model"] ?? effective["ai.model"] ?? effective["model"]);
      setStatus("#aiRuntimeStatus", "#aiHealthText", model || "Local Ollama", model ? "success" : "neutral");
    }).catch(() => setStatus("#aiRuntimeStatus", "#aiHealthText", "Not configured", "warning"));

    void this.api.githubStatus().then((status) => {
      const identity = text(status["username"] ?? status["login"] ?? status["user"]);
      const connected = Boolean(status["connected"] ?? status["authenticated"] ?? identity);
      setStatus("#githubRuntimeStatus", "#githubHealthText", connected ? identity || "Connected" : "Optional", connected ? "success" : "neutral");
    }).catch(() => setStatus("#githubRuntimeStatus", "#githubHealthText", "Optional", "neutral"));
  }'''
    replace_once(path, old, new)


def main() -> None:
    patch_html()
    patch_typescript()
    print("overview subsystem probes decoupled")


if __name__ == "__main__":
    main()
