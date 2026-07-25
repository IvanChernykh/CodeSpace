export class LiveEvents {
    constructor() {
        this.source = null;
        this.handlers = new Set();
        this.statusHandlers = new Set();
    }
    connect(url) {
        this.disconnect();
        const source = new EventSource(url);
        source.onopen = () => this.emitStatus(true);
        source.onerror = () => this.emitStatus(false);
        source.onmessage = (message) => {
            try {
                const parsed = JSON.parse(message.data);
                for (const handler of this.handlers)
                    handler(parsed);
            }
            catch {
            }
        };
        this.source = source;
    }
    disconnect() {
        this.source?.close();
        this.source = null;
    }
    onEvent(handler) {
        this.handlers.add(handler);
        return () => this.handlers.delete(handler);
    }
    onStatusChange(handler) {
        this.statusHandlers.add(handler);
        return () => this.statusHandlers.delete(handler);
    }
    emitStatus(connected) {
        for (const handler of this.statusHandlers)
            handler(connected);
    }
}
