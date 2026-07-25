import { el } from "./dom.js";

export type ToastKind = "info" | "success" | "error" | "warning";

export class ToastManager {
  private readonly container: HTMLElement;

  constructor(container: HTMLElement) {
    this.container = container;
  }

  show(kind: ToastKind, message: string, timeoutMs = 4500): void {
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

  private remove(toast: HTMLElement): void {
    toast.classList.remove("toast-visible");
    window.setTimeout(() => toast.remove(), 200);
  }

  info(message: string): void {
    this.show("info", message);
  }

  success(message: string): void {
    this.show("success", message);
  }

  error(message: string): void {
    this.show("error", message, 7000);
  }

  warning(message: string): void {
    this.show("warning", message);
  }
}

function iconFor(kind: ToastKind): string {
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
