// The home-screen pixel scene, drawn live in layers: twinkling stars, a shimmering sun,
// drifting clouds, swaying trees and grass, flickering lanterns and campfire, fireflies.
// The layers shift a little with the mouse (parallax). Tiny canvases at ~10 fps, paused
// whenever the home screen isn't visible or Minecraft is running, so it costs next to nothing.
(() => {
  const root = document.querySelector(".hero-art");
  if (!root) return;
  const W = 192, H = 96;
  const rnd = (seed) => () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);

  // ── layers (back to front) with their parallax depth in CSS pixels ──
  const layer = (name, depth, w = W) => {
    const wrap = document.createElement("div");
    wrap.className = "hl hl-" + name;
    const c = document.createElement("canvas");
    c.width = w;
    c.height = H;
    wrap.appendChild(c);
    root.appendChild(wrap);
    return { wrap, c, g: c.getContext("2d"), depth };
  };
  const sky = layer("sky", 3);
  const clouds = layer("clouds", 7, W * 2);
  const far = layer("far", 11);
  const near = layer("near", 16);
  const front = layer("front", 24);
  const layers = [sky, clouds, far, near, front];
  const px = (g, x, y, c, w = 1, h = 1) => {
    g.fillStyle = c;
    g.fillRect(x, y, w, h);
  };

  // ── sky: banded gradient, stars, sun ──
  const stops = [[0, [36, 30, 72]], [0.25, [78, 50, 120]], [0.45, [150, 74, 140]], [0.62, [214, 108, 120]], [0.78, [240, 150, 100]], [1, [246, 190, 110]]];
  const skyAt = (t) => {
    for (let i = 0; i < stops.length - 1; i++) {
      const [a, ca] = stops[i], [b, cb] = stops[i + 1];
      if (t <= b) return `rgb(${ca.map((v, k) => Math.round(v + (cb[k] - v) * ((t - a) / (b - a)))).join(",")})`;
    }
    return "rgb(246,190,110)";
  };
  const r1 = rnd(7);
  const stars = Array.from({ length: 44 }, () => ({ x: Math.floor(r1() * W), y: Math.floor(r1() * H * 0.3), p: r1() * 6.28, s: 0.6 + r1() * 1.6, big: r1() < 0.18 }));
  let shoot = null, nextShoot = 6;
  function drawSky(t) {
    const g = sky.g;
    for (let y = 0; y < H; y++) px(g, 0, y, skyAt(Math.round(Math.min(1, y / (H * 0.7)) * 22) / 22), W, 1);
    for (const s of stars) {
      const b = 0.5 + 0.5 * Math.sin(t * s.s + s.p);
      if (b < 0.18) continue; // twinkle: off for a moment
      px(g, s.x, s.y, b > 0.7 ? "#f4eeff" : "#b9a8f0");
      if (s.big && b > 0.85) {
        px(g, s.x - 1, s.y, "#b9a8f0");
        px(g, s.x + 1, s.y, "#b9a8f0");
        px(g, s.x, s.y - 1, "#b9a8f0");
        px(g, s.x, s.y + 1, "#b9a8f0");
      }
    }
    if (shoot) {
      const k = (t - shoot.t0) / 0.7;
      if (k >= 1) shoot = null;
      else for (let i = 0; i < 5; i++) px(g, Math.round(shoot.x + (k * 40 - i * 2)), Math.round(shoot.y + (k * 14 - i * 0.7)), i ? "rgba(233,225,255," + (0.7 - i * 0.14) + ")" : "#ffffff");
    } else if (t > nextShoot) {
      shoot = { t0: t, x: 20 + Math.random() * 90, y: 2 + Math.random() * 10 };
      nextShoot = t + 7 + Math.random() * 9;
    }
    // sun with a dithered halo that slowly breathes
    const cx = 112, cy = 54, r = 11, pulse = Math.sin(t * 0.9) > 0 ? 1 : 0, phase = Math.floor(t * 1.2) % 2;
    for (let y = cy - r - 6; y <= cy + r + 6; y++)
      for (let x = cx - r - 6; x <= cx + r + 6; x++) {
        const d = Math.hypot(x - cx, y - cy);
        if (d <= r - 3) px(g, x, y, "#ffe6a6");
        else if (d <= r) px(g, x, y, "#ffcf7e");
        else if (d <= r + 4 + pulse && (x + y + phase) % 2 === 0) px(g, x, y, "#f8b878");
      }
  }

  // ── clouds: drawn once on a double-width strip that CSS scrolls endlessly ──
  function drawClouds() {
    const g = clouds.g;
    const cloud = (x0, y0, w, col, sh) => {
      for (let i = 0; i < w; i++) {
        const h = 2 + Math.floor(2 * Math.sin((i / w) * Math.PI)) + (i % 5 === 2 ? 1 : 0);
        px(g, x0 + i, y0 - h + 1, col, 1, h);
        px(g, x0 + i, y0 + 1, sh);
      }
    };
    for (const off of [0, W]) {
      cloud(off + 18, 24, 26, "#c98fbf", "#a97aa8");
      cloud(off + 70, 16, 18, "#b886b8", "#9a72a3");
      cloud(off + 150, 34, 22, "#f7c3b4", "#d99a92");
      cloud(off + 96, 38, 14, "#f2b4ae", "#d6948f");
      cloud(off + 126, 10, 12, "#9c78ad", "#86689b");
    }
  }

  // ── mountains ──
  function ridge(g, base, amp, freq, col, seed) {
    const off = rnd(seed)() * 10;
    for (let x = -8; x < W + 8; x++) {
      let h = base - Math.floor(amp * (0.5 + 0.5 * Math.sin(x * freq + off)) * (0.7 + 0.3 * Math.sin(x * freq * 2.7 + off)));
      h -= h % 2;
      px(g, x, h, col, 1, H - h);
    }
  }

  // ── foreground: ground, grass, trees, lanterns, campfire, fireflies ──
  const TREES = [[8, 22], [16, 16], [30, 26], [172, 24], [182, 18], [188, 28], [60, 14], [122, 12], [-3, 20], [196, 22]];
  const r2 = rnd(21);
  const flies = Array.from({ length: 9 }, () => ({ x: 20 + r2() * 150, y: 62 + r2() * 18, p: r2() * 6.28, s: 0.5 + r2() * 0.8 }));
  function drawFront(t) {
    const g = front.g;
    g.clearRect(0, 0, W, H);
    // lantern glow (behind everything on this layer), flickering
    for (const [lx, n] of [[48, 0], [150, 1]]) {
      const f = Math.sin(t * 7 + n * 2) + Math.sin(t * 13.3 + n) * 0.6, rr = f > 0.6 ? 7 : f < -0.9 ? 5 : 6;
      for (let dy = -rr; dy <= rr; dy++)
        for (let dx = -rr; dx <= rr; dx++) {
          const d = Math.hypot(dx, dy);
          if (d < 2.6) px(g, lx + dx, 68 + dy, "rgba(255,206,140,.6)");
          else if (d < 4.6) px(g, lx + dx, 68 + dy, "rgba(246,170,124,.32)");
          else if (d < rr) px(g, lx + dx, 68 + dy, "rgba(240,160,124,.16)");
        }
    }
    // trees: every row leans a little more the higher it is → they sway in the wind
    TREES.forEach(([x0, h], n) => {
      const wind = Math.sin(t * 1.1 + n * 1.7) * 0.75 + Math.sin(t * 2.3 + n) * 0.35;
      for (let y = 0; y < h; y++) {
        const w = 1 + Math.floor((h - y) / 3);
        const lean = Math.round(wind * Math.pow(y / h, 1.6) * 2.2);
        px(g, x0 - w + lean, 84 - y, "#1f1a30", w + 1, 1);
        px(g, x0 + 1 + lean, 84 - y, "#2a2240", w, 1);
      }
    });
    // ground + grass whose tips wave
    for (let x = -8; x < W + 8; x++) {
      const top = 84 + (Math.floor((x + 600) / 6) % 3 === 0 ? 1 : 0);
      px(g, x, top, "#6fa54a");
      px(g, x, top + 1, "#4c7a3a", 1, 2);
      px(g, x, top + 3, "#3b2a2a", 1, H - top - 3);
      if ((x + 600) % 4 === 0) px(g, x + (Math.sin(t * 2.2 + x * 0.35) > 0.35 ? 1 : 0), top - 1, "#6fa54a");
    }
    // lantern posts and lamps
    for (const lx of [48, 150]) {
      px(g, lx, 70, "#2a1e1a", 1, 15);
      px(g, lx - 1, 66, "#ffd36e", 3, 4);
      px(g, lx, 65, "#2a1e1a");
    }
    // campfire: three flame frames + a rising spark
    const fx = 98, fy = 84, fr = Math.floor(t * 8) % 3;
    px(g, fx - 3, fy, "#5b3a22", 7, 1);
    const flames = [
      [[0, -1, "#ffb347"], [0, -2, "#ff8c3a"], [-1, -1, "#ff7a2f"], [1, -1, "#ff7a2f"], [0, -3, "#ffd36e"], [-1, -2, "#ffb347"], [1, -2, "#ffd36e"], [0, -4, "#ff8c3a"]],
      [[0, -1, "#ffd36e"], [0, -2, "#ffb347"], [-1, -1, "#ff8c3a"], [1, -1, "#ffb347"], [0, -3, "#ff8c3a"], [-1, -2, "#ff7a2f"], [1, -2, "#ff8c3a"], [1, -3, "#ffd36e"]],
      [[0, -1, "#ffb347"], [0, -2, "#ffd36e"], [-1, -1, "#ffb347"], [1, -1, "#ff7a2f"], [0, -3, "#ffb347"], [-1, -2, "#ffd36e"], [1, -2, "#ff7a2f"], [-1, -3, "#ff8c3a"], [0, -4, "#ffd36e"]],
    ];
    for (const [dx, dy, c] of flames[fr]) px(g, fx + dx, fy + dy, c);
    const sp = (t * 0.9) % 1;
    px(g, fx + Math.round(Math.sin(t * 5) * 1.5), Math.round(fy - 5 - sp * 9), `rgba(255,190,110,${1 - sp})`);
    // fireflies
    for (const f of flies) {
      const b = Math.sin(t * f.s * 2 + f.p);
      if (b < 0.1) continue;
      px(g, Math.round(f.x + Math.sin(t * f.s + f.p) * 6), Math.round(f.y + Math.cos(t * f.s * 0.7 + f.p) * 3), b > 0.7 ? "#fff3a6" : "rgba(255,230,140,.6)");
    }
  }

  // static layers
  drawClouds();
  ridge(far.g, 66, 20, 0.045, "#5a3a78", 1);
  ridge(far.g, 72, 16, 0.07, "#43305f", 2);
  ridge(near.g, 80, 10, 0.11, "#2e2546", 3);
  drawSky(0);
  drawFront(0);
  root.classList.add("live");

  // ── loop: art at ~10 fps, parallax eased every frame while the mouse moves ──
  const mouse = { x: 0, y: 0, cx: 0, cy: 0, on: false };
  window.addEventListener("pointermove", (e) => {
    mouse.x = (e.clientX / window.innerWidth) * 2 - 1;
    mouse.y = (e.clientY / window.innerHeight) * 2 - 1;
    mouse.on = true;
  }, { passive: true });
  document.addEventListener("pointerleave", () => (mouse.x = mouse.y = 0));
  const active = () =>
    !document.hidden && !document.body.classList.contains("paused") && !document.body.classList.contains("no-motion") &&
    document.getElementById("page-home").classList.contains("active") && (document.getElementById("intro")?.hidden ?? true);
  let last = 0, skyTick = 0;
  function frame(now) {
    requestAnimationFrame(frame);
    if (!active()) return;
    const t = now / 1000;
    if (now - last >= 100) {
      last = now;
      drawFront(t);
      if (++skyTick % 2 === 0) drawSky(t);
    }
    mouse.cx += (mouse.x - mouse.cx) * 0.06;
    mouse.cy += (mouse.y - mouse.cy) * 0.06;
    for (const l of layers) l.wrap.style.transform = `translate3d(${(-mouse.cx * l.depth).toFixed(2)}px, ${(-mouse.cy * l.depth * 0.45).toFixed(2)}px, 0)`;
  }
  requestAnimationFrame(frame);
})();
