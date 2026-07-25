export function qs(selector, root = document) {
    const el = root.querySelector(selector);
    if (!el) {
        throw new Error(`element not found: ${selector}`);
    }
    return el;
}
export function qsa(selector, root = document) {
    return Array.from(root.querySelectorAll(selector));
}
export function el(tag, attrs = {}, children = []) {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) {
        if (value === undefined || value === false)
            continue;
        if (key.startsWith("on") && typeof value === "function") {
            node.addEventListener(key.slice(2).toLowerCase(), value);
        }
        else if (key === "class") {
            node.className = String(value);
        }
        else if (value === true) {
            node.setAttribute(key, "");
        }
        else {
            node.setAttribute(key, String(value));
        }
    }
    for (const child of children) {
        node.append(typeof child === "string" ? document.createTextNode(child) : child);
    }
    return node;
}
const escapeMap = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
};
export function escapeHtml(value) {
    return String(value ?? "").replace(/[&<>"']/g, (ch) => escapeMap[ch] ?? ch);
}
export function debounce(fn, waitMs) {
    let handle;
    return (...args) => {
        if (handle !== undefined)
            window.clearTimeout(handle);
        handle = window.setTimeout(() => fn(...args), waitMs);
    };
}
export function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
}
export function formatBytes(bytes) {
    if (bytes < 1024)
        return `${bytes} B`;
    const units = ["KB", "MB", "GB"];
    let value = bytes / 1024;
    let unitIndex = 0;
    while (value >= 1024 && unitIndex < units.length - 1) {
        value /= 1024;
        unitIndex += 1;
    }
    return `${value.toFixed(1)} ${units[unitIndex]}`;
}
export function formatRelativeTime(unixMs) {
    const deltaSeconds = Math.round((Date.now() - unixMs) / 1000);
    if (deltaSeconds < 5)
        return "just now";
    if (deltaSeconds < 60)
        return `${deltaSeconds}s ago`;
    const minutes = Math.round(deltaSeconds / 60);
    if (minutes < 60)
        return `${minutes}m ago`;
    const hours = Math.round(minutes / 60);
    if (hours < 24)
        return `${hours}h ago`;
    const days = Math.round(hours / 24);
    return `${days}d ago`;
}
export function formatTimestamp(unixMs) {
    return new Date(unixMs).toLocaleString();
}
export async function copyToClipboard(text) {
    try {
        await navigator.clipboard.writeText(text);
        return true;
    }
    catch {
        return false;
    }
}
