// Thin bridge to the Rust backend. When opened in a normal browser (UI development,
// screenshots) it falls back to a mock that talks to the public Modrinth API directly.
(() => {
  const T = window.__TAURI__;
  if (T && T.core) {
    window.api = {
      call: (cmd, args) => T.core.invoke(cmd, args),
      on: (ev, cb) => T.event.listen(ev, (e) => cb(e.payload)),
      native: true,
    };
    return;
  }

  const listeners = {};
  const emit = (ev, p) => (listeners[ev] || []).forEach((cb) => cb(p));
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  let settings = {
    language: localStorage.getItem("mock-lang") || "ru",
    gameVersion: "", memoryMb: 4096, gc: "balanced", extraJvmArgs: "", javaPath: "",
    autoJoin: true, closeOnLaunch: false, curseforgeKey: "", width: 1280, height: 720, reduceMotion: false,
  };
  let account = new URLSearchParams(location.search).has("guest") ? null : { name: "StrayZii", uuid: "" };
  const installed = [
    { source: "modrinth", projectId: "AANobbMI", slug: "sodium", title: "Sodium", version: "0.9.3", fileName: "sodium.jar", iconUrl: null, enabled: true, locked: true },
    { source: "modrinth", projectId: "uXXizFIs", slug: "ferrite-core", title: "FerriteCore", version: "9.0.0", fileName: "ferritecore.jar", iconUrl: null, enabled: true, locked: true },
  ];
  const MR = "https://api.modrinth.com/v2";

  const handlers = {
    async get_init() {
      return { settings, account, totalMemoryMb: 16384, hasClientId: true, hasCfKey: false, appVersion: "0.1.0", os: "macos", dataDir: "~/Library/Application Support/StrayLauncher" };
    },
    async save_settings({ settings: s }) { settings = s; localStorage.setItem("mock-lang", s.language); return !!s.curseforgeKey; },
    async game_versions() { return ["26.3", "26.2", "26.1.2", "26.1.1", "26.1", "1.21.11", "1.21.10", "1.21.8", "1.21.4", "1.21.1", "1.20.6", "1.20.4", "1.20.1"]; },
    async server_status() { await wait(400); return { online: true, players: 137, max: 2026, version: "Velocity 1.20-26.3", ping_ms: 38 }; },
    async login_window() {
      setTimeout(() => emit("auth-busy", {}), 3500);
      setTimeout(() => emit("auth", { ok: true, account: { name: "StrayZii", uuid: "" } }), 5000);
    },
    async login_cancel() {},
    async login_start() {
      setTimeout(() => emit("auth", { ok: true, account: { name: "StrayZii", uuid: "" } }), 6000);
      return { userCode: "SM7Y-2QKD", verificationUri: "https://www.microsoft.com/link" };
    },
    async logout() { account = null; },
    async mods_list() {
      const missing = installed.filter((m) => !m.iconUrl);
      if (missing.length) {
        const ids = encodeURIComponent(JSON.stringify(missing.map((m) => m.projectId)));
        const ps = await fetch(`${MR}/projects?ids=${ids}`).then((r) => r.json()).catch(() => []);
        for (const p of ps) { const m = installed.find((m) => m.projectId === p.id); if (m) m.iconUrl = p.icon_url; }
      }
      return installed.slice();
    },
    async mods_search({ source, query, game, offset }) {
      if (source === "curseforge") throw "cf_no_key";
      const facets = encodeURIComponent(JSON.stringify([["project_type:mod"], ["categories:fabric"], [`versions:${game}`]]));
      const r = await fetch(`${MR}/search?query=${encodeURIComponent(query)}&facets=${facets}&limit=24&offset=${offset}&index=${query ? "relevance" : "downloads"}`);
      const j = await r.json();
      const have = new Set(installed.map((m) => m.projectId));
      return j.hits.map((h) => ({ source: "modrinth", projectId: h.project_id, slug: h.slug, title: h.title, description: h.description, author: h.author, downloads: h.downloads, iconUrl: h.icon_url, installed: have.has(h.project_id) }));
    },
    async mod_install({ projectId }) {
      for (let i = 1; i <= 4; i++) { await wait(150); emit("progress", { stage: "mods", done: i, total: 4 }); }
      const r = await fetch(`${MR}/project/${projectId}`).then((r) => r.json());
      installed.push({ source: "modrinth", projectId: r.id, slug: r.slug, title: r.title, version: "latest", fileName: r.slug + ".jar", iconUrl: r.icon_url, enabled: true, locked: false });
      return [r.title];
    },
    async mod_toggle({ projectId, enabled }) { const m = installed.find((m) => m.projectId === projectId); if (m) m.enabled = enabled; },
    async mod_remove({ projectId }) { const i = installed.findIndex((m) => m.projectId === projectId); if (i >= 0) installed.splice(i, 1); },
    async open_dir() {},
    async open_url({ url }) { window.open(url, "_blank"); },
    async play() {
      if (!account) throw "not logged in";
      const stages = [["meta", 1], ["java", 147], ["libraries", 74], ["assets", 5147], ["mods", 2]];
      for (const [stage, total] of stages) for (let i = 0; i <= 10; i++) { await wait(60); emit("progress", { stage, done: Math.round((total * i) / 10), total }); }
      emit("game", { state: "running" });
      setTimeout(() => emit("game", { state: "exited", code: 0 }), 8000);
    },
  };

  window.api = {
    call: async (cmd, args = {}) => handlers[cmd](args),
    on: (ev, cb) => { (listeners[ev] ||= []).push(cb); },
    native: false,
  };
})();
