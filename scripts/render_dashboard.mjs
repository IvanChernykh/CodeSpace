#!/usr/bin/env node

import { writeFile } from "node:fs/promises";

const [pageUrl, screenshotPath, domPath, errorPath] = process.argv.slice(2);
if (!pageUrl || !screenshotPath || !domPath || !errorPath) {
  console.error("usage: render_dashboard.mjs <url> <screenshot.png> <dom.html> <error.txt>");
  process.exit(2);
}

const debugPort = Number.parseInt(process.env.CHROME_DEBUG_PORT ?? "9222", 10);
const targetListUrl = `http://127.0.0.1:${debugPort}/json/list`;
const deadline = Date.now() + 30_000;
let lastState = null;

const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

async function findPageTarget() {
  while (Date.now() < deadline) {
    try {
      const response = await fetch(targetListUrl, { signal: AbortSignal.timeout(2_000) });
      if (response.ok) {
        const targets = await response.json();
        const target = targets.find((candidate) => candidate.type === "page" && candidate.url.startsWith(pageUrl));
        if (target?.webSocketDebuggerUrl) return target;
      }
    } catch {
      // Chrome may still be opening its DevTools endpoint.
    }
    await sleep(100);
  }
  throw new Error(`Chrome DevTools target did not appear at ${targetListUrl}`);
}

async function connect(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", () => reject(new Error("Chrome DevTools WebSocket failed to open")), { once: true });
  });

  let nextId = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(String(event.data));
    if (!message.id) return;
    const operation = pending.get(message.id);
    if (!operation) return;
    pending.delete(message.id);
    if (message.error) operation.reject(new Error(`${operation.method}: ${message.error.message}`));
    else operation.resolve(message.result);
  });
  socket.addEventListener("close", () => {
    for (const operation of pending.values()) operation.reject(new Error("Chrome DevTools WebSocket closed"));
    pending.clear();
  });

  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++nextId;
    pending.set(id, { method, resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });

  return { socket, send };
}

async function evaluate(send, expression) {
  const evaluation = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (evaluation.exceptionDetails) {
    throw new Error(`browser evaluation failed: ${evaluation.exceptionDetails.text ?? "unknown error"}`);
  }
  return evaluation.result?.value;
}

async function main() {
  const target = await findPageTarget();
  const { socket, send } = await connect(target.webSocketDebuggerUrl);
  try {
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setDeviceMetricsOverride", {
      width: 1440,
      height: 1000,
      deviceScaleFactor: 1,
      mobile: false,
    });

    while (Date.now() < deadline) {
      lastState = await evaluate(send, `(() => {
        const chip = document.querySelector("#connectionChip");
        return {
          readyState: document.readyState,
          title: document.title,
          connection: chip?.dataset.state ?? null,
          commandCenter: document.body?.innerText.includes("Command Center") ?? false,
          topologyNodes: document.querySelectorAll("[data-overview-file]").length,
        };
      })()`);
      if (
        lastState?.readyState === "complete"
        && lastState?.title === "CodeSpace — IDE Assistant"
        && lastState?.connection === "online"
        && lastState?.commandCenter === true
        && lastState?.topologyNodes > 0
      ) {
        break;
      }
      await sleep(200);
    }

    if (lastState?.connection !== "online" || lastState?.topologyNodes <= 0) {
      throw new Error(`dashboard did not become render-ready: ${JSON.stringify(lastState)}`);
    }

    await evaluate(send, "document.fonts?.ready ?? Promise.resolve()");
    await sleep(250);
    const html = await evaluate(send, "document.documentElement.outerHTML");
    if (typeof html !== "string" || !html.includes("data-state=\"online\"")) {
      throw new Error("rendered DOM did not contain the online connection state");
    }
    await writeFile(domPath, `<!doctype html>\n${html}\n`, "utf8");

    const capture = await send("Page.captureScreenshot", {
      format: "png",
      fromSurface: true,
      captureBeyondViewport: false,
    });
    if (!capture.data) throw new Error("Chrome returned an empty screenshot");
    await writeFile(screenshotPath, Buffer.from(capture.data, "base64"));
    console.log(JSON.stringify({ pageUrl, ...lastState }));
  } finally {
    socket.close();
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? `${error.stack ?? error.message}\nlast_state=${JSON.stringify(lastState)}\n` : `${String(error)}\n`;
  await writeFile(errorPath, message, "utf8");
  console.error(message);
  process.exit(1);
}
