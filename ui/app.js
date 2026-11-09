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
    $("#skin-import-name").placeholder = t("skin.importPh");
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
    if (s.startsWith("skin_bad_size:")) return t("err.skin_bad_size", s.slice(14).replace("x", "×"));
    if (s.startsWith("skin_no_player:")) return t("err.skin_no_player", s.slice(15));
    if (s.startsWith("skin_upload_failed:")) return t("err.skin_upload_failed", s.slice(19));
    if (s.startsWith("cape_bad_size:")) return t("err.cape_bad_size", s.slice(14).replace("x", "×"));
    if (s.startsWith("keystrokes_unavailable:")) return t("ks.unavailable", s.slice(23));
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
    if (page === "modules") loadModules();
    if (page === "capes") loadWardrobe();
  }
  $$("[data-page]").forEach((b) => b.addEventListener("click", () => go(b.dataset.page)));
  document.addEventListener("click", (e) => {
    const u = e.target.closest("[data-url]");
    if (u) api.call("open_url", { url: u.dataset.url });
  });

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

  const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

  // ───────── accounts (Lunar-style switcher) ─────────
  const headUrl = (a, size) => `https://mc-heads.net/avatar/${a.uuid || a.name}/${size}`;
  function setAccounts(list) {
    S.accounts = list || { active: null, accounts: [] };
    S.init.account = S.accounts.accounts.find((a) => a.uuid === S.accounts.active) || null;
    renderAccount();
  }
  function renderAccount() {
    const btn = $("#account");
    const acc = S.init?.account;
    btn.classList.toggle("guest", !acc);
    $("#acc-name").textContent = acc ? acc.name : t("acc.login");
    const head = $("#acc-head");
    head.onerror = () => (head.src = "assets/logo.svg");
    if (acc) head.src = headUrl(acc, 28);
    const menu = $("#acc-menu");
    if (!menu.hidden) renderAccountMenu();
  }
  function renderAccountMenu() {
    const menu = $("#acc-menu");
    const list = S.accounts?.accounts || [];
    menu.innerHTML =
      `<div class="acc-menu-title">${t("acc.accounts")}</div>` +
      list
        .map(
          (a) => `<div class="acc-row ${a.uuid === S.accounts.active ? "active" : ""}" data-uuid="${esc(a.uuid)}">
        <img src="${esc(headUrl(a, 64))}" alt="" width="32" height="32" onerror="this.src='assets/logo.svg'" />
        <span class="acc-row-name">${esc(a.name)}<small>${a.uuid === S.accounts.active ? t("acc.active") : t("acc.switch")}</small></span>
        <button class="acc-remove" data-remove="${esc(a.uuid)}" title="${t("acc.remove")}">✕</button>
      </div>`
        )
        .join("") +
      `<button class="acc-add" id="acc-add"><b>+</b> ${t("acc.add")}</button>`;
  }
  $("#account").addEventListener("click", (e) => {
    e.stopPropagation();
    if (!S.accounts?.accounts?.length) return login();
    const menu = $("#acc-menu");
    menu.hidden = !menu.hidden;
    if (!menu.hidden) renderAccountMenu();
  });
  $("#acc-menu").addEventListener("click", async (e) => {
    e.stopPropagation();
    const rm = e.target.closest("[data-remove]");
    if (rm) {
      setAccounts(await api.call("account_remove", { uuid: rm.dataset.remove }));
      if (!S.accounts.accounts.length) $("#acc-menu").hidden = true;
      return;
    }
    if (e.target.closest("#acc-add")) {
      $("#acc-menu").hidden = true;
      return login();
    }
    const row = e.target.closest(".acc-row");
    if (row && row.dataset.uuid !== S.accounts.active) {
      setAccounts(await api.call("account_switch", { uuid: row.dataset.uuid }));
      $("#acc-menu").hidden = true;
      toast(t("acc.ok", S.init.account.name));
    }
  });
  document.addEventListener("click", () => ($("#acc-menu").hidden = true));

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
      api.call("accounts_list").then(setAccounts);
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
      if (api.demo) toast(t("demo.play"));
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
        .join("") +
      (userMods
        ? ""
        : `<div class="empty-state">
            <svg viewBox="0 0 24 24"><path d="M12 2 3 7v10l9 5 9-5V7zM12 12 3 7m9 5 9-5m-9 5v10"/></svg>
            <p>${t("mods.none")}</p>
            <button class="btn btn-soft" data-goto-tab="modrinth">${t("mods.findMore")}</button>
          </div>`);
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
    else
      grid.innerHTML =
        html ||
        `<div class="empty-state"><svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg><p>${t("mods.empty")}</p></div>`;
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
    const goto = e.target.closest("[data-goto-tab]");
    if (goto) return $(`.tab[data-tab="${goto.dataset.gotoTab}"]`).click();
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
    const fillRange = () => mem.style.setProperty("--fill", `${((mem.value - mem.min) / (mem.max - mem.min)) * 100}%`);
    fillRange();
    mem.addEventListener("input", () => {
      fillRange();
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
    $("#app-version").textContent = `Stray Client v${S.init.appVersion} · ${S.init.os}${S.init.arch ? " " + S.init.arch : ""}`;
  }

  // ───────── boot ─────────
  // ───────── modules (keystrokes HUD) ─────────
  let applyTimer;
  function applyModulesSoon() {
    clearTimeout(applyTimer);
    applyTimer = setTimeout(async () => {
      await saveSettings();
      try {
        await api.call("modules_apply");
        S.installed = await api.call("mods_list", { game: currentGame() }).catch(() => S.installed);
        renderPerf();
      } catch (e) {
        toast(errText(e), true);
      }
    }, 450);
  }
  async function availability() {
    return api.call("modules_availability", { game: currentGame() }).catch(() => ({ capes: true, keystrokes: true }));
  }
  function renderKeystrokes() {
    const ks = S.settings.keystrokes;
    $("#ks-enabled").checked = ks.enabled;
    $(".module-card").classList.toggle("off", !ks.enabled);
    $$("[data-ks]").forEach((el) => (el.checked = !!ks[el.dataset.ks]));
    const pv = $("#ks-preview");
    pv.querySelectorAll(".ks-keys").forEach((r) => r.classList.toggle("hide", !ks.keys));
    pv.querySelector(".ks-mouse").classList.toggle("hide", !ks.mouse);
    pv.querySelector(".ks-space").classList.toggle("hide", !ks.space);
    pv.querySelector(".ks-fps").classList.toggle("hide", !ks.fps);
    pv.querySelector(".ks-ping").classList.toggle("hide", !ks.ping);
    pv.querySelector(".ks-extra").classList.toggle("hide", !ks.fps && !ks.ping);
  }
  async function loadModules() {
    renderKeystrokes();
    const av = await availability();
    const n = $("#ks-notice");
    n.hidden = av.keystrokes;
    n.textContent = av.keystrokes ? "" : t("ks.unavailable", currentGame());
  }
  $("#ks-enabled").addEventListener("change", (e) => {
    S.settings.keystrokes.enabled = e.target.checked;
    renderKeystrokes();
    applyModulesSoon();
  });
  $$("[data-ks]").forEach((el) =>
    el.addEventListener("change", () => {
      S.settings.keystrokes[el.dataset.ks] = el.checked;
      renderKeystrokes();
      applyModulesSoon();
    })
  );
  // Live preview: real key presses and clicks light up the HUD, with a real CPS counter.
  const clicks = { 0: [], 2: [] };
  const keyEl = (code) => $(`#ks-preview [data-k="${code}"]`);
  const typing = (e) => /INPUT|TEXTAREA|SELECT/.test(e.target.tagName);
  document.addEventListener("keydown", (e) => {
    if (!$("#page-modules").classList.contains("active") || typing(e)) return;
    const el = keyEl(e.code);
    if (el) {
      el.classList.add("down");
      if (e.code === "Space") e.preventDefault();
    }
  });
  document.addEventListener("keyup", (e) => keyEl(e.code)?.classList.remove("down"));
  const ksPreview = $("#ks-preview");
  ksPreview.addEventListener("contextmenu", (e) => e.preventDefault());
  ksPreview.addEventListener("mousedown", (e) => {
    const b = ksPreview.querySelector(`[data-m="${e.button}"]`);
    if (!b) return;
    b.classList.add("down");
    clicks[e.button].push(performance.now());
  });
  document.addEventListener("mouseup", (e) => ksPreview.querySelector(`[data-m="${e.button}"]`)?.classList.remove("down"));
  setInterval(() => {
    if (!$("#page-modules").classList.contains("active")) return;
    const now = performance.now();
    for (const b of [0, 2]) {
      clicks[b] = clicks[b].filter((t0) => now - t0 < 1000);
      $(b === 0 ? "#cps-l" : "#cps-r").textContent = `${clicks[b].length} CPS`;
    }
  }, 100);

  // ───────── capes ─────────
  S.capes = [];
  function drawCape(canvas, src, side = "front") {
    const ctx = canvas.getContext("2d");
    if (!src) {
      canvas.width = 10;
      canvas.height = 16;
      ctx.clearRect(0, 0, 10, 16);
      return;
    }
    const img = new Image();
    img.onload = () => {
      const u = img.width === 22 ? 1 : img.width / 64; // 64×32 layout (22×17 = old OptiFine)
      canvas.width = Math.round(10 * u);
      canvas.height = Math.round(16 * u);
      ctx.imageSmoothingEnabled = false;
      const sx = (side === "front" ? 1 : 12) * u;
      ctx.drawImage(img, sx, u, 10 * u, 16 * u, 0, 0, canvas.width, canvas.height);
    };
    img.src = src;
  }
  function renderCapes() {
    const sel = S.settings.capeMode === "local" ? S.capes.find((c) => c.file === S.settings.cape) : null;
    drawCape($("#cape-front"), sel?.dataUrl, "front");
    drawCape($("#cape-back"), sel?.dataUrl, "back");
    $("#cape-name").textContent = sel
      ? sel.file.replace(/\.png$/, "")
      : S.settings.capeMode === "none"
        ? t("cape.hidden")
        : S.settings.capeMode === "account"
          ? t("cape.accountName")
          : t("cape.noneSelected");
    $("#cape-size").textContent = sel ? `${sel.width}×${sel.height}` : "";
    $$("#cape-mode button").forEach((b) => b.classList.toggle("on", b.dataset.v === S.settings.capeMode));
    const grid = $("#cape-grid");
    grid.innerHTML =
      S.capes
        .map(
          (c) => `<div class="cape-card ${sel && sel.file === c.file ? "on" : ""}" data-file="${esc(c.file)}">
          <button class="cape-del" data-del="${esc(c.file)}" title="${t("mods.remove")}">✕</button>
          <canvas></canvas><span>${esc(c.file.replace(/\.png$/, ""))}</span></div>`
        )
        .join("") +
      `<label class="cape-card add"><b>+</b><span>${t("cape.add")}</span><input type="file" accept="image/png" multiple hidden /></label>`;
    grid.querySelectorAll(".cape-card[data-file]").forEach((card) => {
      const c = S.capes.find((x) => x.file === card.dataset.file);
      drawCape(card.querySelector("canvas"), c.dataUrl, "front");
    });
    grid.querySelector(".cape-card.add input").addEventListener("change", (e) => addCapeFiles(e.target.files));
  }
  async function loadCapes() {
    S.capes = await api.call("capes_list").catch(() => []);
    renderCapes();
    const av = await availability();
    const n = $("#cape-notice");
    n.hidden = av.capes;
    n.textContent = av.capes ? "" : t("cape.unsupported", currentGame());
  }
  const readB64 = (file) =>
    new Promise((res, rej) => {
      const r = new FileReader();
      r.onload = () => res(String(r.result).split(",")[1] || "");
      r.onerror = rej;
      r.readAsDataURL(file);
    });
  async function addCapeFiles(files) {
    let last = null;
    for (const f of files) {
      try {
        last = await api.call("cape_add", { name: f.name, data: await readB64(f) });
        toast(t("cape.added", last.replace(/\.png$/, "")));
      } catch (e) {
        toast(errText(e), true);
      }
    }
    if (last) {
      S.settings.capeMode = "local";
      S.settings.cape = last;
      applyModulesSoon();
    }
    loadCapes();
  }
  $("#cape-file").addEventListener("change", (e) => {
    addCapeFiles(e.target.files);
    e.target.value = "";
  });
  $("#cape-mode").addEventListener("click", (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    S.settings.capeMode = b.dataset.v;
    if (b.dataset.v === "local" && !S.settings.cape && S.capes[0]) S.settings.cape = S.capes[0].file;
    renderCapes();
    applyModulesSoon();
  });
  $("#cape-grid").addEventListener("click", async (e) => {
    const del = e.target.closest("[data-del]");
    if (del) {
      e.stopPropagation();
      await api.call("cape_remove", { file: del.dataset.del });
      if (S.settings.cape === del.dataset.del) {
        S.settings.cape = "";
        S.settings.capeMode = "account";
        applyModulesSoon();
      }
      return loadCapes();
    }
    const card = e.target.closest(".cape-card[data-file]");
    if (!card) return;
    S.settings.capeMode = "local";
    S.settings.cape = card.dataset.file;
    renderCapes();
    applyModulesSoon();
  });
  const capeDrop = $("#cape-drop");
  ["dragenter", "dragover"].forEach((ev) =>
    capeDrop.addEventListener(ev, (e) => {
      e.preventDefault();
      capeDrop.classList.add("drag");
    })
  );
  ["dragleave", "drop"].forEach((ev) =>
    capeDrop.addEventListener(ev, (e) => {
      e.preventDefault();
      capeDrop.classList.remove("drag");
    })
  );
  capeDrop.addEventListener("drop", (e) => addCapeFiles(e.dataTransfer.files));

  // ───────── wardrobe: skins ─────────
  S.skins = [];
  S.skinSel = null; // library file, or "__current" for the account skin
  S.currentSkin = null;
  $("#wr-tabs").addEventListener("click", (e) => {
    const b = e.target.closest("[data-wr]");
    if (!b) return;
    $$("#wr-tabs .tab").forEach((x) => x.classList.toggle("active", x === b));
    $("#wr-skin").hidden = b.dataset.wr !== "skin";
    $("#wr-cape").hidden = b.dataset.wr !== "cape";
    if (b.dataset.wr === "cape") loadCapes();
  });
  async function loadWardrobe() {
    if (!$("#wr-cape").hidden) loadCapes();
    await loadSkins();
  }

  // Draws the front or back of a skin as a flat 16×32 figure (64×64 and legacy 64×32 layouts).
  function drawSkin(canvas, src, variant = "classic", side = "front") {
    const ctx = canvas.getContext("2d");
    canvas.width = 16;
    canvas.height = 32;
    ctx.clearRect(0, 0, 16, 32);
    if (!src) return;
    const img = new Image();
    img.onload = () => {
      ctx.imageSmoothingEnabled = false;
      const legacy = img.height === 32;
      const aw = variant === "slim" ? 3 : 4;
      const part = (sx, sy, w, h, dx, dy, mirror = false) => {
        if (!mirror) return ctx.drawImage(img, sx, sy, w, h, dx, dy, w, h);
        ctx.save();
        ctx.translate(dx + w, dy);
        ctx.scale(-1, 1);
        ctx.drawImage(img, sx, sy, w, h, 0, 0, w, h);
        ctx.restore();
      };
      if (side === "front") {
        part(8, 8, 8, 8, 4, 0);                                   // head
        part(20, 20, 8, 12, 4, 8);                                // body
        part(44, 20, aw, 12, 4 - aw, 8);                          // right arm
        part(4, 20, 4, 12, 4, 20);                                // right leg
        if (legacy) {
          part(44, 20, aw, 12, 12, 8, true);
          part(4, 20, 4, 12, 8, 20, true);
        } else {
          part(36, 52, aw, 12, 12, 8);                            // left arm
          part(20, 52, 4, 12, 8, 20);                             // left leg
          part(20, 36, 8, 12, 4, 8);                              // jacket
          part(44, 36, aw, 12, 4 - aw, 8);                        // sleeves
          part(52, 52, aw, 12, 12, 8);
          part(4, 36, 4, 12, 4, 20);                              // trousers
          part(4, 52, 4, 12, 8, 20);
        }
        part(40, 8, 8, 8, 4, 0);                                  // hat layer
      } else {
        part(24, 8, 8, 8, 4, 0);
        part(32, 20, 8, 12, 4, 8);
        part(48 + aw, 20, aw, 12, 12, 8);                         // right arm back (now on the right)
        part(12, 20, 4, 12, 8, 20);
        if (legacy) {
          part(48 + aw, 20, aw, 12, 4 - aw, 8, true);
          part(12, 20, 4, 12, 4, 20, true);
        } else {
          part(40 + aw, 52, aw, 12, 4 - aw, 8);                   // left arm back
          part(28, 52, 4, 12, 4, 20);                             // left leg
          part(32, 36, 8, 12, 4, 8);
          part(48 + aw, 36, aw, 12, 12, 8);
          part(56 + aw, 52, aw, 12, 4 - aw, 8);
          part(12, 36, 4, 12, 8, 20);
          part(12, 52, 4, 12, 4, 20);
        }
        part(56, 8, 8, 8, 4, 0);
      }
    };
    img.src = src;
  }

  function selectedSkin() {
    if (S.skinSel === "__current") return S.currentSkin?.dataUrl ? { file: "__current", variant: S.currentSkin.variant, dataUrl: S.currentSkin.dataUrl, current: true } : null;
    return S.skins.find((x) => x.file === S.skinSel) || null;
  }
  function renderSkins() {
    if (!S.skinSel || (S.skinSel !== "__current" && !S.skins.some((x) => x.file === S.skinSel))) {
      S.skinSel = S.currentSkin?.dataUrl ? "__current" : S.skins[0]?.file || null;
    }
    const sel = selectedSkin();
    drawSkin($("#skin-front"), sel?.dataUrl, sel?.variant, "front");
    drawSkin($("#skin-back"), sel?.dataUrl, sel?.variant, "back");
    $("#skin-name").textContent = sel ? (sel.current ? t("skin.current") : sel.file.replace(/\.png$/, "")) : "";
    $("#skin-size").textContent = sel && !sel.current ? `${sel.width}×${sel.height}` : "";
    $$("#skin-variant button").forEach((b) => {
      b.classList.toggle("on", sel?.variant === b.dataset.v);
      b.disabled = !sel || !!sel.current;
    });
    $("#skin-apply").disabled = !sel || !!sel.current;
    $("#skin-reset").hidden = !S.init.account;
    const cards = [];
    if (S.currentSkin?.dataUrl) {
      cards.push(`<div class="cape-card ${S.skinSel === "__current" ? "on" : ""}" data-skin="__current">
        <em class="badge-mini">${t("skin.badge")}</em><canvas class="skin-thumb"></canvas><span>${esc(S.init.account?.name || "")}</span></div>`);
    }
    for (const x of S.skins) {
      cards.push(`<div class="cape-card ${S.skinSel === x.file ? "on" : ""}" data-skin="${esc(x.file)}">
        <button class="cape-del" data-skin-del="${esc(x.file)}" title="${t("mods.remove")}">✕</button>
        <canvas class="skin-thumb"></canvas><span>${esc(x.file.replace(/\.png$/, ""))}</span></div>`);
    }
    cards.push(`<label class="cape-card add"><b>+</b><span>${t("skin.add")}</span><input type="file" accept="image/png" multiple hidden /></label>`);
    const grid = $("#skin-grid");
    grid.innerHTML = cards.join("");
    grid.querySelectorAll("[data-skin]").forEach((card) => {
      const f = card.dataset.skin;
      const x = f === "__current" ? S.currentSkin : S.skins.find((s) => s.file === f);
      drawSkin(card.querySelector("canvas"), x.dataUrl, x.variant, "front");
    });
    grid.querySelector(".cape-card.add input").addEventListener("change", (e) => addSkinFiles(e.target.files));
  }
  async function loadSkins() {
    const [list, cur] = await Promise.all([
      api.call("skins_list").catch(() => []),
      S.init.account ? api.call("skin_current").catch(() => null) : Promise.resolve(null),
    ]);
    S.skins = list;
    S.currentSkin = cur;
    renderSkins();
    const n = $("#skin-notice");
    n.hidden = !!S.init.account;
    n.textContent = S.init.account ? "" : t("skin.needLogin");
  }
  async function addSkinFiles(files) {
    let last = null;
    for (const f of files) {
      try {
        last = await api.call("skin_add", { name: f.name, data: await readB64(f), variant: "classic" });
        toast(t("skin.imported", last.replace(/\.png$/, "")));
      } catch (e) {
        toast(errText(e), true);
      }
    }
    if (last) S.skinSel = last;
    loadSkins();
  }
  $("#skin-file").addEventListener("change", (e) => {
    addSkinFiles(e.target.files);
    e.target.value = "";
  });
  $("#skin-grid").addEventListener("click", async (e) => {
    const del = e.target.closest("[data-skin-del]");
    if (del) {
      e.stopPropagation();
      await api.call("skin_remove", { file: del.dataset.skinDel }).catch((err) => toast(errText(err), true));
      return loadSkins();
    }
    const card = e.target.closest("[data-skin]");
    if (!card) return;
    S.skinSel = card.dataset.skin;
    renderSkins();
  });
  $("#skin-variant").addEventListener("click", async (e) => {
    const b = e.target.closest("button");
    const sel = selectedSkin();
    if (!b || !sel || sel.current) return;
    sel.variant = b.dataset.v;
    renderSkins();
    await api.call("skin_variant", { file: sel.file, variant: sel.variant }).catch((err) => toast(errText(err), true));
  });
  $("#skin-apply").addEventListener("click", async () => {
    const sel = selectedSkin();
    if (!sel || sel.current) return;
    if (!S.init.account) return login();
    const btn = $("#skin-apply");
    btn.disabled = true;
    btn.firstElementChild.textContent = t("skin.applying");
    try {
      await api.call("skin_apply", { file: sel.file });
      toast(t("skin.applied"));
      S.skinSel = "__current";
      await loadSkins();
    } catch (err) {
      toast(errText(err), true);
    }
    btn.firstElementChild.textContent = t("skin.apply");
    btn.disabled = false;
  });
  $("#skin-reset").addEventListener("click", async () => {
    try {
      await api.call("skin_reset");
      toast(t("skin.resetDone"));
      await loadSkins();
    } catch (err) {
      toast(errText(err), true);
    }
  });
  async function importSkin() {
    const name = $("#skin-import-name").value.trim();
    if (!name) return;
    const btn = $("#skin-import");
    btn.disabled = true;
    try {
      const file = await api.call("skin_import", { name });
      toast(t("skin.imported", name));
      $("#skin-import-name").value = "";
      S.skinSel = file;
      await loadSkins();
    } catch (err) {
      toast(errText(err), true);
    }
    btn.disabled = false;
  }
  $("#skin-import").addEventListener("click", importSkin);
  $("#skin-import-name").addEventListener("keydown", (e) => e.key === "Enter" && importSkin());
  const skinDrop = $("#skin-drop");
  ["dragenter", "dragover"].forEach((ev) =>
    skinDrop.addEventListener(ev, (e) => {
      e.preventDefault();
      skinDrop.classList.add("drag");
    })
  );
  ["dragleave", "drop"].forEach((ev) =>
    skinDrop.addEventListener(ev, (e) => {
      e.preventDefault();
      skinDrop.classList.remove("drag");
    })
  );
  skinDrop.addEventListener("drop", (e) => addSkinFiles(e.dataTransfer.files));

  if (api.demo && !new URLSearchParams(location.search).has("shot")) {
    const b = document.createElement("em");
    b.className = "demo-badge";
    b.textContent = "DEMO";
    $(".brand").appendChild(b);
  }

  async function boot() {
    S.init = await api.call("get_init");
    S.settings = S.init.settings;
    S.settings.keystrokes ||= { enabled: false, keys: true, mouse: true, space: true, fps: false, ping: false };
    S.settings.capeMode ||= "account";
    S.settings.cape ||= "";
    S.accounts = S.init.accounts || { active: null, accounts: S.init.account ? [S.init.account] : [] };
    document.body.classList.toggle("no-motion", S.settings.reduceMotion);
    bindSettings();
    applyI18n();
    S.versions = await api.call("game_versions").catch(() => []);
    renderVersionSelects();
    S.installed = await api.call("mods_list", { game: currentGame() }).catch(() => []);
    renderPerf();
    const page = new URLSearchParams(location.search).get("page");
    if (page) go(page);
  }
  boot();
})();
