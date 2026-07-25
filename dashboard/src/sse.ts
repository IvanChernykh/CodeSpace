import type { SseEvent } from "./types.js";

type EventHandler = (event: SseEvent) => void;
type StatusHandler = (connected: boolean) => void;

/** Wraps native EventSource with typed payloads and automatic reconnect handled by the browser. */
export class LiveEvents {
  private source: EventSource | null = null;
  private readonly handlers = new Set<EventHandler>();
  private readonly statusHandlers = new Set<StatusHandler>();

  connect(url: string): void {
    this.disconnect();
    const source = new EventSource(url);
    source.onopen = () => this.emitStatus(true);
    source.onerror = () => this.emitStatus(false);
    source.onmessage = (message) => {
      try {
        const parsed = JSON.parse(message.data) as SseEvent;
        for (const handler of this.handlers) handler(parsed);
      } catch {
        // Ignore malformed/heartbeat frames.
      }
    };
    this.source = source;
  }

  disconnect(): void {
    this.source?.close();
    this.source = null;
  }

  onEvent(handler: EventHandler): () => void {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }

  onStatusChange(handler: StatusHandler): () => void {
    this.statusHandlers.add(handler);
    return () => this.statusHandlers.delete(handler);
  }

  private emitStatus(connected: boolean): void {
    for (const handler of this.statusHandlers) handler(connected);
  }
}
