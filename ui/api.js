// Thin bridge to the Rust backend. Opened in a normal browser it becomes the interactive
// HTML demo (website, screenshots): everything works except actually starting Minecraft.
// Mod search uses the public Modrinth API for real.
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
  const params = new URLSearchParams(location.search);
  const store = {
    get: (k) => { try { return localStorage.getItem(k); } catch { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch {} },
  };
  const urlLang = params.get("lang");
  let settings = {
    language: ["ru", "uk", "en"].includes(urlLang) ? urlLang : store.get("mock-lang") || (/^uk/i.test(navigator.language) ? "uk" : /^(ru|be|kk)/i.test(navigator.language) ? "ru" : "en"),
    gameVersion: "", memoryMb: 4096, gc: "balanced", extraJvmArgs: "", javaPath: "",
    autoJoin: true, closeOnLaunch: false, curseforgeKey: "", width: 1280, height: 720, reduceMotion: false,
    capeMode: "local", cape: "straysmp.png",
    // Like the real client until Mojang approves direct sign-in: play through the official launcher.
    playMode: "official",
    keystrokes: { enabled: true, keys: true, mouse: true, space: true, fps: true, ping: false },
  };
  // Demo cape library (the real client keeps PNGs in its data folder).
  let skins = [{ file: "straysmp.png", variant: "classic", width: 64, height: 64, dataUrl: "assets/demo-skin.png" }];
  let accountSkin = { variant: "classic", dataUrl: "assets/demo-skin.png" };
  const blobB64 = (blob) => new Promise((res) => { const r = new FileReader(); r.onload = () => res(String(r.result).split(",")[1]); r.readAsDataURL(blob); });
  let capes = [{ file: "straysmp.png", width: 64, height: 32, dataUrl: "assets/demo-cape.png" }];
  const b64Size = (b64) => {
    const bin = atob(b64.slice(0, 64));
    const u = (i) => ((bin.charCodeAt(i) << 24) | (bin.charCodeAt(i + 1) << 16) | (bin.charCodeAt(i + 2) << 8) | bin.charCodeAt(i + 3)) >>> 0;
    return bin.slice(1, 4) === "PNG" && bin.slice(12, 16) === "IHDR" ? [u(16), u(20)] : null;
  };
  // Demo accounts for the Lunar-style switcher.
  let accounts = params.has("guest") ? [] : [{ name: "StrayZii", uuid: "" }, { name: "Alex", uuid: "" }];
  let active = accounts[0]?.name ?? null;
  const list = () => ({ active, accounts: accounts.map((a) => ({ name: a.name, uuid: a.name })) });
  const current = () => accounts.find((a) => a.name === active) || null;
  let demoN = 0;
  const installed = [
    { source: "modrinth", projectId: "AANobbMI", slug: "sodium", title: "Sodium", version: "0.9.3", fileName: "sodium.jar", iconUrl: null, enabled: true, locked: true },
    { source: "modrinth", projectId: "uXXizFIs", slug: "ferrite-core", title: "FerriteCore", version: "9.0.0", fileName: "ferritecore.jar", iconUrl: null, enabled: true, locked: true },
  ];
  const MR = "https://api.modrinth.com/v2";

  const handlers = {
    async get_init() {
      const playMode = settings.playMode === "official" ? "official" : "direct";
      return { playMode, official: { launcherFound: true }, settings, account: current() && { name: active, uuid: active }, accounts: list(), totalMemoryMb: 16384, hasClientId: true, hasCfKey: false, appVersion: "1.2.0", os: "macos", dataDir: "~/Library/Application Support/StrayLauncher" };
    },
    async save_settings({ settings: s }) { settings = s; store.set("mock-lang", s.language); return !!s.curseforgeKey; },
    async game_versions() { return ["26.3", "26.2", "26.1.2", "26.1.1", "26.1", "1.21.11", "1.21.10", "1.21.8", "1.21.4", "1.21.1", "1.20.6", "1.20.4", "1.20.1"]; },
    async server_status() { await wait(400); return { online: true, players: 137, max: 2026, version: "Velocity 1.20-26.3", ping_ms: 38 }; },
    // Demo sign-in: the real client opens Microsoft's page in its own window here.
    async login_window() {
      const name = accounts.length ? `Player${(++demoN) * 137 % 900 + 100}` : "StrayZii";
      setTimeout(() => emit("auth-busy", {}), 2500);
      setTimeout(() => {
        if (!accounts.some((a) => a.name === name)) accounts.push({ name, uuid: "" });
        active = name;
        emit("auth", { ok: true, account: { name, uuid: name } });
      }, 4000);
    },
    async login_cancel() {},
    async login_start() {
      setTimeout(() => {
        if (!accounts.length) accounts.push({ name: "StrayZii", uuid: "" });
        active = accounts[0].name;
        emit("auth", { ok: true, account: { name: active, uuid: active } });
      }, 6000);
      return { userCode: "SM7Y-2QKD", verificationUri: "https://www.microsoft.com/link" };
    },
    async logout() { accounts = accounts.filter((a) => a.name !== active); active = accounts[0]?.name ?? null; },
    async accounts_list() { return list(); },
    async account_switch({ uuid }) { active = uuid; return list(); },
    async account_remove({ uuid }) {
      accounts = accounts.filter((a) => a.name !== uuid);
      if (active === uuid) active = accounts[0]?.name ?? null;
      return list();
    },
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
    async skins_list() { return skins.map((x) => ({ ...x })); },
    async skin_add({ name, data, variant }) {
      const size = b64Size(data);
      if (!size) throw "skin_not_png";
      const [w, h] = size;
      if (!(w === 64 && (h === 64 || h === 32))) throw `skin_bad_size:${w}x${h}`;
      let base = name.replace(/\.png$/i, "").toLowerCase().replace(/[^a-z0-9_-]/g, "_").slice(0, 40) || "skin";
      let file = `${base}.png`, n = 2;
      while (skins.some((x) => x.file === file)) file = `${base}-${n++}.png`;
      skins.push({ file, variant, width: w, height: h, dataUrl: `data:image/png;base64,${data}` });
      return file;
    },
    async skin_variant({ file, variant }) { const x = skins.find((y) => y.file === file); if (x) x.variant = variant; },
    async skin_remove({ file }) { skins = skins.filter((x) => x.file !== file); },
    async skin_current() { return { ...accountSkin }; },
    async skin_apply({ file }) {
      if (!current()) throw "not logged in";
      await wait(1200);
      const x = skins.find((y) => y.file === file);
      accountSkin = { variant: x.variant, dataUrl: x.dataUrl };
    },
    async skin_reset() { await wait(500); accountSkin = { variant: "classic", dataUrl: null }; },
    // Demo: public skin mirror (the real client asks Mojang's API directly).
    async skin_import({ name }) {
      if (!/^[A-Za-z0-9_]{1,16}$/.test(name)) throw "skin_bad_name";
      const r = await fetch(`https://mc-heads.net/skin/${encodeURIComponent(name)}`).catch(() => null);
      if (!r || !r.ok) throw `skin_no_player:${name}`;
      return handlers.skin_add({ name, data: await blobB64(await r.blob()), variant: "classic" });
    },
    async capes_list() { return capes.slice(); },
    async cape_add({ name, data }) {
      const size = b64Size(data);
      if (!size) throw "cape_not_png";
      const [w, h] = size;
      if (!((w === 2 * h && w >= 32 && w <= 4096) || (w === 22 && h === 17))) throw `cape_bad_size:${w}x${h}`;
      let base = name.replace(/\.png$/i, "").toLowerCase().replace(/[^a-z0-9_-]/g, "_").slice(0, 40) || "cape";
      let file = `${base}.png`, n = 2;
      while (capes.some((c) => c.file === file)) file = `${base}-${n++}.png`;
      capes.push({ file, width: w, height: h, dataUrl: `data:image/png;base64,${data}` });
      return file;
    },
    async cape_remove({ file }) { capes = capes.filter((c) => c.file !== file); },
    async modules_availability({ game }) { return { capes: parseInt(game, 10) >= 26, keystrokes: true, pvpPlus: game === "26.3" || game.startsWith("26.3."), extras: Object.fromEntries(["freelook","appleskin","chat-heads","shulkerboxtooltip","mouse-tweaks","dynamic-fps","xaeros-minimap","xaeros-world-map","replaymod","lambdynamiclights"].map((id) => [id, parseInt(game, 10) >= 26])) }; },
    async modules_apply() {},
    async open_dir() {},
    async open_url({ url }) { window.open(url, "_blank"); },
    async official_status() { return { launcherFound: true, minecraftDir: "~/Library/Application Support/minecraft" }; },
    async play() {
      if (settings.playMode === "official") {
        for (const [stage, total] of [["meta", 1], ["mods", 2]]) for (let i = 0; i <= 6; i++) { await wait(70); emit("progress", { stage, done: Math.round((total * i) / 6), total }); }
        emit("game", { state: "official", version: "fabric-loader-0.20.1-26.3" });
        return;
      }
      if (!current()) throw "not logged in";
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
    demo: true,
  };
})();
