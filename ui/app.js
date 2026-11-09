(() => {
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const api = window.api;

  const PERF = [
    { slug: "lithium", title: "Lithium" },
    { slug: "immediatelyfast", title: "ImmediatelyFast" },
    { slug: "entityculling", title: "EntityCulling" },
    { slug: "modernfix", title: "ModernFix" },
  ];

  const S = {
    init: null,
    settings: null,
    versions: [],
    installed: [],
    tab: "installed",
    query: "",
    offset: 0,
    busy: false,
    running: false,
    installing: new Set(),
  };

  // ───────── i18n ─────────
  const t = (key, ...args) => {
    const dict = I18N[S.settings?.language] || I18N.ru;
    let s = dict[key] ?? I18N.en[key] ?? key;
    args.forEach((a, i) => (s = s.replace(`{${i}}`, a)));
    return s;
  };
  function applyI18n() {
    document.documentElement.lang = S.settings.language;
    $$("[data-i18n]").forEach((el) => (el.textContent = t(el.dataset.i18n)));
    $("#search").placeholder = t("mods.search");
    $$(".lang button").forEach((b) => b.classList.toggle("on", b.dataset.lang === S.settings.language));
    $$("#set-lang button").forEach((b) => b.classList.toggle("on", b.dataset.v === S.settings.language));
    renderVersionSelects();
    renderAccount();
    updateMemHint();
    renderStatus();
    if (!S.busy && !S.running) $("#play-label").textContent = t("play");
  }

  function errText(e) {
    let s = String(e?.message ?? e);
    s = s.replace(/^auth: /, "");
    if (I18N.en["err." + s]) return t("err." + s);
    if (s === "not logged in") return t("acc.need");
    if (s === "cf_no_key") return t("mods.cfKey");
    if (s.startsWith("no_version:")) return `${s.slice(11)}: ${S.settings.language === "ru" ? "нет версии для" : "no build for"} ${currentGame()}`;
    if (s.startsWith("cf_no_download:")) return `${s.slice(15)}: ${S.settings.language === "ru" ? "автор запретил сторонние загрузки" : "author disabled third-party downloads"}`;
    return t("err.generic", s);
  }

  let toastTimer;
  function toast(msg, err = false) {
    const el = $("#toast");
    el.textContent = msg;
    el.classList.toggle("err", err);
    el.hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (el.hidden = true), err ? 5200 : 2600);
  }

  // ───────── navigation ─────────
  function go(page) {
    $$(".page").forEach((p) => p.classList.toggle("active", p.id === "page-" + page));
    $$(".nav-btn").forEach((b) => b.classList.toggle("active", b.dataset.page === page));
    if (page === "mods") loadMods();
  }
  $$("[data-page]").forEach((b) => b.addEventListener("click", () => go(b.dataset.page)));
  document.addEventListener("click", (e) => {
    const u = e.target.closest("[data-url]");
    if (u) api.call("open_url", { url: u.dataset.url });
  });

  // ───────── leaves (cheap: 12 elements, transform-only animation) ─────────
  function makeLeaves() {
    const box = $("#leaves");
    const path = '<svg viewBox="0 0 16 16"><path d="M8 0c3 3 6 5 6 9a6 6 0 0 1-5 6v1H7v-1a6 6 0 0 1-5-6c0-4 3-6 6-9z"/></svg>';
    for (let i = 0; i < 12; i++) {
      const l = document.createElement("i");
      l.className = "leaf";
      l.innerHTML = path;
      const dur = 11 + Math.random() * 10;
      l.style.cssText = `left:${Math.random() * 100}%;animation-duration:${dur}s;animation-delay:${-Math.random() * dur}s;` +
        `--dx:${(Math.random() * 160 - 60) | 0}px;--rot:${(Math.random() * 540 - 180) | 0}deg;` +
        `transform:scale(${0.6 + Math.random() * 0.7});opacity:${0.35 + Math.random() * 0.45}`;
      box.appendChild(l);
    }
  }
  // Nothing animates in the background: pause when hidden or while Minecraft runs.
  const syncPause = () => document.body.classList.toggle("paused", document.hidden || S.running);
  document.addEventListener("visibilitychange", syncPause);

  // ───────── client status (hero cards) ─────────
  function renderStatus() {
    if (!S.init) return;
    const gb = (mb) => (mb / 1024).toFixed(mb % 1024 ? 1 : 0) + " GB";
    const total = S.init.totalMemoryMb || 0;
    $("#mem-use").textContent = gb(S.settings.memoryMb);
    $("#mem-total").textContent = total ? ` / ${gb(total)}` : "";
    $("#mem-bar").style.width = total ? Math.max(2, Math.min(100, (S.settings.memoryMb / total) * 100)) + "%" : "0";
    $("#gc-label").textContent = S.settings.gc === "lowlatency" ? "ZGC" : "G1";
    const n = S.installed.length || 2;
    let word;
    if (S.settings.language === "ru") {
      const m10 = n % 10, m100 = n % 100;
      word = m10 === 1 && m100 !== 11 ? "мод" : m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14) ? "мода" : "модов";
    } else word = n === 1 ? "mod" : "mods";
    $("#mods-chip-label").textContent = t("modsCount", n, word);
  }

  // ───────── versions ─────────
  const currentGame = () => S.settings.gameVersion || S.versions[0] || "";
  function renderVersionSelects() {
    for (const sel of [$("#version-select"), $("#set-version")]) {
      const latest = S.versions[0] ? `${t("version.latest")} (${S.versions[0]})` : t("version.latest");
      sel.innerHTML = `<option value="">${latest}</option>` + S.versions.map((v) => `<option>${v}</option>`).join("");
      sel.value = S.settings.gameVersion;
    }
  }
  for (const sel of [$("#version-select"), $("#set-version")]) {
    sel.addEventListener("change", async () => {
      S.settings.gameVersion = sel.value;
      renderVersionSelects();
      await saveSettings();
      S.installed = await api.call("mods_list", { game: currentGame() }).catch(() => []);
      renderPerf();
      if (S.tab === "installed") renderInstalled();
    });
  }

  // ───────── account ─────────
  function renderAccount() {
    const btn = $("#account");
    const acc = S.init?.account;
    btn.classList.toggle("guest", !acc);
    $("#acc-name").textContent = acc ? acc.name : t("acc.login");
    const head = $("#acc-head");
    head.onerror = () => (head.src = "assets/logo.svg");
    if (acc) head.src = `https://mc-heads.net/avatar/${acc.uuid || acc.name}/28`;
  }
  $("#account").addEventListener("click", (e) => {
    e.stopPropagation();
    if (!S.init.account) return login();
    let pop = $(".acc-pop");
    if (pop) return pop.remove();
    pop = document.createElement("div");
    pop.className = "acc-pop";
    pop.innerHTML = `<button>${t("acc.logout")}</button>`;
    pop.firstChild.onclick = async () => {
      await api.call("logout");
      S.init.account = null;
      pop.remove();
      renderAccount();
    };
    $(".top-right").style.position = "relative";
    $(".top-right").appendChild(pop);
  });
  document.addEventListener("click", () => $(".acc-pop")?.remove());

  // Sign-in: Microsoft's page opens in its own window (like Lunar Client).
  // The device-code flow stays available as a fallback.
  let loginUri = "";
  const modal = $("#login-modal");
  function showLoginCard(which) {
    $("#login-window-card").hidden = which !== "window";
    $("#login-code-card").hidden = which !== "code";
    modal.hidden = false;
  }
  async function login() {
    $("#login-window-state").textContent = t("acc.waitingWindow");
    showLoginCard("window");
    try {
      await api.call("login_window");
    } catch (e) {
      modal.hidden = true;
      toast(errText(e), true);
    }
  }
  async function loginWithCode() {
    try {
      const code = await api.call("login_start");
      loginUri = code.verificationUri;
      $("#login-code").textContent = code.userCode;
      showLoginCard("code");
      try { await navigator.clipboard.writeText(code.userCode); } catch {}
    } catch (e) {
      modal.hidden = true;
      toast(errText(e), true);
    }
  }
  $("#login-use-code").addEventListener("click", () => {
    api.call("login_cancel");
    loginWithCode();
  });
  $("#login-open").addEventListener("click", () => api.call("open_url", { url: loginUri }));
  $("#login-code").addEventListener("click", async () => {
    try { await navigator.clipboard.writeText($("#login-code").textContent); } catch {}
    toast(t("copy"));
  });
  $("#login-cancel").addEventListener("click", () => (modal.hidden = true));
  $("#login-cancel-w").addEventListener("click", () => {
    modal.hidden = true;
    api.call("login_cancel");
  });
  api.on("auth-busy", () => {
    $("#login-window-state").textContent = t("acc.checking");
    showLoginCard("window");
  });
  api.on("auth", (r) => {
    if (r.ok) {
      modal.hidden = true;
      S.init.account = r.account;
      renderAccount();
      toast(t("acc.ok", r.account.name));
      return;
    }
    if (r.error === "cancelled" || modal.hidden) {
      // Window closed: only hide the dialog if the user isn't using the code fallback.
      if (!$("#login-window-card").hidden) modal.hidden = true;
      return;
    }
    modal.hidden = true;
    toast(errText(r.error), true);
  });

  // ───────── play ─────────
  const playBtn = $("#play");
  function setPlay(state, label) {
    playBtn.classList.toggle("busy", state === "busy");
    playBtn.classList.toggle("running", state === "running");
    $("#play-label").textContent = label;
    if (state !== "busy") $("#play-fill").style.width = "0";
  }
  api.on("progress", (p) => {
    if (!S.busy) return;
    const pct = p.total ? Math.round((p.done / p.total) * 100) : 0;
    $("#play-fill").style.width = pct + "%";
    $("#stage").textContent = `${t("stage." + p.stage)} · ${p.done}/${p.total}`;
  });
  api.on("game", (g) => {
    if (g.state === "running") {
      S.busy = false;
      S.running = true;
      setPlay("running", t("play.running"));
      $("#stage").textContent = "";
    } else {
      S.running = false;
      setPlay("idle", t("play"));
      if (g.code && g.code !== 0) toast(t("game.crashed", g.code), true);
    }
    syncPause();
  });
  playBtn.addEventListener("click", async () => {
    if (S.busy || S.running) return;
    if (!S.init.account) return login();
    S.busy = true;
    setPlay("busy", t("play.preparing"));
    try {
      await api.call("play");
    } catch (e) {
      S.busy = false;
      setPlay("idle", t("play"));
      $("#stage").textContent = "";
      toast(errText(e), true);
    }
  });

  // ───────── performance extras on home ─────────
  function renderPerf() {
    renderStatus();
    const have = new Set(S.installed.map((m) => m.slug));
    $("#perf-extras").innerHTML = PERF.map(
      (p) => `<button class="extra ${have.has(p.slug) ? "on" : ""} ${S.installing.has(p.slug) ? "busy" : ""}" data-slug="${p.slug}">${p.title}</button>`
    ).join("");
  }
  $("#perf-extras").addEventListener("click", async (e) => {
    const b = e.target.closest(".extra");
    if (!b) return;
    const slug = b.dataset.slug;
    const mod = S.installed.find((m) => m.slug === slug);
    S.installing.add(slug);
    renderPerf();
    try {
      if (mod) await api.call("mod_remove", { game: currentGame(), projectId: mod.projectId });
      else {
        const t2 = await api.call("mod_install", { source: "modrinth", projectId: slug, game: currentGame() });
        toast(t("mods.done", t2.join(", ")));
      }
      S.installed = await api.call("mods_list", { game: currentGame() });
    } catch (err) {
      toast(errText(err), true);
    }
    S.installing.delete(slug);
    renderPerf();
  });

  // ───────── mods page ─────────
  const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const fmt = (n) => (n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? Math.round(n / 1e3) + "K" : n);
  const ICON_FALLBACK = "data:image/svg+xml," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1" fill="#2f3040"/></svg>');
  const trash = '<svg viewBox="0 0 24 24"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/></svg>';

  function renderInstalled() {
    $("#installed-count").textContent = S.installed.length;
    const grid = $("#mod-grid");
    const userMods = S.installed.filter((m) => !m.locked).length;
    grid.innerHTML =
      S.installed
        .map(
          (m) => `<div class="mod" data-id="${esc(m.projectId)}">
        <img class="mod-icon" loading="lazy" src="${esc(m.iconUrl || ICON_FALLBACK)}" alt="" />
        <div class="mod-body">
          <div class="mod-title"><span>${esc(m.title)}</span>${m.locked ? `<em class="lock">${t("mods.core")}</em>` : ""}</div>
          <div class="mod-meta">${esc(m.version)} · ${m.source === "modrinth" ? "Modrinth" : "CurseForge"}</div>
        </div>
        <div class="mod-actions">
          ${m.locked ? "" : `<button class="icon-btn" data-remove title="${t("mods.remove")}">${trash}</button>`}
          <label class="toggle"><input type="checkbox" data-toggle ${m.enabled ? "checked" : ""} ${m.locked ? "disabled" : ""} /><i></i></label>
        </div>
      </div>`
        )
        .join("") + (userMods ? "" : `<div class="empty">${t("mods.none")}</div>`);
    $("#more").hidden = true;
  }

  function renderHits(hits, append) {
    const grid = $("#mod-grid");
    const html = hits
      .map(
        (h) => `<div class="mod" data-id="${esc(h.projectId)}" data-source="${h.source}">
      <img class="mod-icon" loading="lazy" src="${esc(h.iconUrl || ICON_FALLBACK)}" alt="" />
      <div class="mod-body">
        <div class="mod-title"><span>${esc(h.title)}</span></div>
        <div class="mod-desc">${esc(h.description)}</div>
        <div class="mod-meta">${t("mods.by")} ${esc(h.author)} · ${fmt(h.downloads)} ${t("mods.downloads")}</div>
      </div>
      <div class="mod-actions">
        ${h.installed ? `<button class="btn btn-ghost" disabled>${t("mods.isInstalled")}</button>` : `<button class="btn btn-primary" data-install>${t("mods.install")}</button>`}
      </div>
    </div>`
      )
      .join("");
    if (append) grid.insertAdjacentHTML("beforeend", html);
    else grid.innerHTML = html || `<div class="empty">${t("mods.empty")}</div>`;
    $("#more").hidden = hits.length < 24;
  }

  let searchSeq = 0;
  async function loadMods(append = false) {
    $("#mods-notice").hidden = true;
    $("#search-wrap").classList.toggle("disabled", S.tab === "installed");
    if (S.tab === "installed") {
      S.installed = await api.call("mods_list", { game: currentGame() }).catch(() => S.installed);
      return renderInstalled();
    }
    if (!append) {
      S.offset = 0;
      $("#mod-grid").innerHTML = '<div class="skeleton"></div>'.repeat(6);
    }
    const seq = ++searchSeq;
    try {
      const hits = await api.call("mods_search", { source: S.tab, query: S.query, game: currentGame(), offset: S.offset });
      if (seq !== searchSeq) return; // a newer search started
      renderHits(hits, append);
    } catch (e) {
      if (seq !== searchSeq) return;
      $("#mod-grid").innerHTML = "";
      const n = $("#mods-notice");
      n.textContent = errText(e);
      n.hidden = false;
    }
  }

  $("#mod-tabs").addEventListener("click", (e) => {
    const b = e.target.closest(".tab");
    if (!b) return;
    S.tab = b.dataset.tab;
    $$(".tab").forEach((x) => x.classList.toggle("active", x === b));
    loadMods();
  });
  let debounce;
  $("#search").addEventListener("input", (e) => {
    S.query = e.target.value.trim();
    clearTimeout(debounce);
    debounce = setTimeout(() => loadMods(), 280);
  });
  $("#more").addEventListener("click", () => {
    S.offset += 24;
    loadMods(true);
  });
  $("#open-mods").addEventListener("click", () => api.call("open_dir", { kind: "mods", game: currentGame() }));

  $("#mod-grid").addEventListener("click", async (e) => {
    const card = e.target.closest(".mod");
    if (!card) return;
    const id = card.dataset.id;
    if (e.target.closest("[data-install]")) {
      const btn = e.target.closest("[data-install]");
      btn.disabled = true;
      btn.textContent = t("mods.installing");
      try {
        const titles = await api.call("mod_install", { source: card.dataset.source, projectId: id, game: currentGame() });
        btn.className = "btn btn-ghost";
        btn.textContent = t("mods.isInstalled");
        toast(t("mods.done", titles.join(", ")));
        S.installed = await api.call("mods_list", { game: currentGame() });
        renderPerf();
      } catch (err) {
        btn.disabled = false;
        btn.textContent = t("mods.install");
        toast(errText(err), true);
      }
    } else if (e.target.closest("[data-remove]")) {
      await api.call("mod_remove", { game: currentGame(), projectId: id }).catch((err) => toast(errText(err), true));
      loadMods();
      renderPerf();
    }
  });
  $("#mod-grid").addEventListener("change", async (e) => {
    if (!e.target.matches("[data-toggle]")) return;
    const id = e.target.closest(".mod").dataset.id;
    try {
      await api.call("mod_toggle", { game: currentGame(), projectId: id, enabled: e.target.checked });
    } catch (err) {
      e.target.checked = !e.target.checked;
      toast(errText(err), true);
    }
  });

  // ───────── settings ─────────
  let saveTimer;
  async function saveSettings() {
    try {
      S.init.hasCfKey = await api.call("save_settings", { settings: S.settings });
    } catch (e) {
      toast(errText(e), true);
    }
  }
  const saveSoon = () => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(saveSettings, 400);
  };

  function recommendedMem() {
    const total = S.init.totalMemoryMb;
    return total >= 16000 ? 6144 : total >= 8000 ? 4096 : 3072;
  }
  function updateMemHint() {
    if (!S.init) return;
    const gb = (mb) => (mb / 1024).toFixed(mb % 1024 ? 1 : 0) + " GB";
    $("#mem-label").textContent = gb(S.settings.memoryMb);
    $("#mem-hint").textContent = t("set.memoryHint", gb(recommendedMem()));
  }

  function bindSettings() {
    const s = S.settings;
    const mem = $("#set-mem");
    mem.max = Math.max(4096, Math.floor((S.init.totalMemoryMb * 0.75) / 512) * 512);
    mem.value = s.memoryMb;
    mem.addEventListener("input", () => {
      s.memoryMb = +mem.value;
      updateMemHint();
      renderStatus();
      saveSoon();
    });
    const checks = { "#set-autojoin": "autoJoin", "#set-close": "closeOnLaunch", "#set-motion": "reduceMotion" };
    for (const [sel, key] of Object.entries(checks)) {
      const el = $(sel);
      el.checked = !!s[key];
      el.addEventListener("change", () => {
        s[key] = el.checked;
        document.body.classList.toggle("no-motion", s.reduceMotion);
        saveSoon();
      });
    }
    const texts = { "#set-cf": "curseforgeKey", "#set-java": "javaPath", "#set-jvm": "extraJvmArgs" };
    for (const [sel, key] of Object.entries(texts)) {
      const el = $(sel);
      el.value = s[key] || "";
      el.addEventListener("input", () => {
        s[key] = el.value;
        saveSoon();
      });
    }
    for (const [sel, key] of [["#set-w", "width"], ["#set-h", "height"]]) {
      const el = $(sel);
      el.value = s[key];
      el.addEventListener("change", () => {
        s[key] = Math.max(320, +el.value || 0);
        saveSoon();
      });
    }
    const seg = (root, key, after) => {
      const sync = () => $$("button", root).forEach((b) => b.classList.toggle("on", b.dataset.v === s[key]));
      sync();
      root.addEventListener("click", (e) => {
        const b = e.target.closest("button");
        if (!b) return;
        s[key] = b.dataset.v;
        sync();
        after?.();
        renderStatus();
        saveSoon();
      });
    };
    seg($("#set-gc"), "gc");
    seg($("#set-lang"), "language", applyI18n);
    $$(".lang button").forEach((b) =>
      b.addEventListener("click", () => {
        s.language = b.dataset.lang;
        applyI18n();
        $$("#set-lang button").forEach((x) => x.classList.toggle("on", x.dataset.v === s.language));
        saveSoon();
      })
    );
    $$("[data-open]").forEach((b) => b.addEventListener("click", () => api.call("open_dir", { kind: b.dataset.open, game: currentGame() })));
    $("#data-dir").textContent = S.init.dataDir;
    $("#app-version").textContent = `Stray Launcher v${S.init.appVersion} · ${S.init.os}`;
  }

  // ───────── boot ─────────
  async function boot() {
    S.init = await api.call("get_init");
    S.settings = S.init.settings;
    document.body.classList.toggle("no-motion", S.settings.reduceMotion);
    bindSettings();
    applyI18n();
    makeLeaves();
    S.versions = await api.call("game_versions").catch(() => []);
    renderVersionSelects();
    S.installed = await api.call("mods_list", { game: currentGame() }).catch(() => []);
    renderPerf();
    const page = new URLSearchParams(location.search).get("page");
    if (page) go(page);
  }
  boot();
})();
