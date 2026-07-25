export class ApiError extends Error {
    constructor(status, message) {
        super(message);
        this.status = status;
        this.name = "ApiError";
    }
}
export class ApiClient {
    constructor(token, base = "/api/v1") {
        this.token = token;
        this.base = base;
    }
    eventsUrl() {
        return `${this.base}/events?token=${encodeURIComponent(this.token)}`;
    }
    async request(path, init = {}) {
        const headers = new Headers(init.headers);
        headers.set("Authorization", `Bearer ${this.token}`);
        if (init.body) {
            headers.set("Content-Type", "application/json");
        }
        let response;
        try {
            response = await fetch(`${this.base}${path}`, { ...init, headers });
        }
        catch {
            throw new ApiError(0, "Network error: is the CodeSpace server running?");
        }
        const text = await response.text();
        let parsed = undefined;
        if (text.length > 0) {
            try {
                parsed = JSON.parse(text);
            }
            catch {
                throw new ApiError(response.status, "Invalid JSON response from server");
            }
        }
        if (!response.ok) {
            const message = parsed && typeof parsed === "object" && parsed !== null && "error" in parsed
                ? String(parsed.error)
                : `Request failed with status ${response.status}`;
            throw new ApiError(response.status, message);
        }
        return parsed;
    }
    qs(params) {
        const search = new URLSearchParams();
        for (const [key, value] of Object.entries(params)) {
            if (value !== undefined && value !== "") {
                search.set(key, String(value));
            }
        }
        const s = search.toString();
        return s.length > 0 ? `?${s}` : "";
    }
    health() {
        return this.request("/health");
    }
    listActions() {
        return this.request("/actions");
    }
    graph() {
        return this.request("/graph");
    }
    search(query, opts = {}) {
        return this.request(`/search${this.qs({ q: query, ...opts })}`);
    }
    context(query, opts = {}) {
        return this.request(`/context${this.qs({ q: query, ...opts })}`);
    }
    stats() {
        return this.request("/stats");
    }
    impact(from, to, depth) {
        return this.request(`/impact${this.qs({ from, to, depth })}`);
    }
    history(query, limit = 25) {
        return this.request(`/history${this.qs({ q: query, limit })}`);
    }
    remember(input) {
        return this.request(`/remember${this.qs({ ...input })}`, { method: "POST" });
    }
    readFile(path, maxLines = 400) {
        return this.request(`/read${this.qs({ file: path, max_lines: maxLines })}`);
    }
    updateIndex(force = false) {
        return this.request(`/update${this.qs({ force })}`, { method: "POST" });
    }
    doctor(repair = false) {
        return this.request(`/doctor${this.qs({ repair })}`, { method: "POST" });
    }
    workspaces() {
        return this.request("/workspaces");
    }
    registerWorkspace(path, name) {
        return this.request(`/workspaces/register${this.qs({ path, name })}`, { method: "POST" });
    }
    selectWorkspace(id) {
        return this.request(`/workspaces/select${this.qs({ id })}`, { method: "POST" });
    }
    removeWorkspace(id) {
        return this.request(`/workspaces/remove${this.qs({ id })}`, { method: "POST" });
    }
}
