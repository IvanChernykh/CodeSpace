import { el } from "./dom.js";
export class ToastManager {
    constructor(container) {
        this.container = container;
    }
    show(kind, message, timeoutMs = 4500) {
        const toast = el("div", { class: `toast toast-${kind}` }, [
            el("span", { class: "toast-icon" }, [iconFor(kind)]),
            el("span", { class: "toast-message" }, [message]),
        ]);
        const dismiss = el("button", { class: "toast-dismiss", "aria-label": "Dismiss" }, ["\u00d7"]);
        dismiss.addEventListener("click", () => this.remove(toast));
        toast.append(dismiss);
        this.container.append(toast);
        requestAnimationFrame(() => toast.classList.add("toast-visible"));
        if (timeoutMs > 0) {
            window.setTimeout(() => this.remove(toast), timeoutMs);
        }
    }
    remove(toast) {
        toast.classList.remove("toast-visible");
        window.setTimeout(() => toast.remove(), 200);
    }
    info(message) {
        this.show("info", message);
    }
    success(message) {
        this.show("success", message);
    }
    error(message) {
        this.show("error", message, 7000);
    }
    warning(message) {
        this.show("warning", message);
    }
}
function iconFor(kind) {
    switch (kind) {
        case "success":
            return "\u2713";
        case "error":
            return "\u2715";
        case "warning":
            return "\u26a0";
        default:
            return "\u24d8";
    }
}
