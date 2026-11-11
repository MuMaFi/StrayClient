// Start-up sounds, synthesised with the Web Audio API (no audio files):
//  • a bit-crushed e-piano running up a major scale while the letters appear,
//  • a coin "ding" with an echo when the logo is complete,
//  • a whoosh when the app opens.
// `StraySfx.build(ctx)` works on any (also an offline) audio context so it can be tested.
window.StraySfx = (() => {
  function build(ctx) {
    const master = ctx.createGain();
    master.gain.value = 0.42;
    master.connect(ctx.destination);

    // bit crusher: quantise the signal to a few steps (the "8-bit" grit)
    const crush = ctx.createWaveShaper();
    const curve = new Float32Array(2048), steps = 7;
    for (let i = 0; i < curve.length; i++) curve[i] = Math.round(((i / (curve.length - 1)) * 2 - 1) * steps) / steps;
    crush.curve = curve;
    const tone = ctx.createBiquadFilter();
    tone.type = "lowpass";
    tone.frequency.value = 5200;
    crush.connect(tone).connect(master);

    // e-piano: a sine carrier with a decaying FM "tine" and a short bell partial
    function piano(freq, t, len = 0.55, vol = 0.5) {
      const car = ctx.createOscillator(), mod = ctx.createOscillator(), idx = ctx.createGain(), amp = ctx.createGain();
      const bell = ctx.createOscillator(), bellAmp = ctx.createGain();
      car.frequency.value = freq;
      mod.frequency.value = freq * 2;
      idx.gain.setValueAtTime(freq * 2.2, t);
      idx.gain.exponentialRampToValueAtTime(freq * 0.15, t + 0.3);
      mod.connect(idx).connect(car.frequency);
      amp.gain.setValueAtTime(0.0001, t);
      amp.gain.exponentialRampToValueAtTime(vol, t + 0.006);
      amp.gain.exponentialRampToValueAtTime(0.0008, t + len);
      bell.frequency.value = freq * 4;
      bellAmp.gain.setValueAtTime(vol * 0.25, t);
      bellAmp.gain.exponentialRampToValueAtTime(0.0005, t + 0.09);
      car.connect(amp).connect(crush);
      bell.connect(bellAmp).connect(crush);
      for (const o of [car, mod, bell]) {
        o.start(t);
        o.stop(t + len + 0.05);
      }
    }
    /** Eight notes up the C major scale, the last one ringing out. */
    function scale(t, step = 0.088) {
      const root = 523.25; // C5
      [1, 9 / 8, 5 / 4, 4 / 3, 3 / 2, 5 / 3, 15 / 8, 2].forEach((r, i) => piano(root * r, t + i * step, i === 7 ? 0.9 : 0.42, i === 7 ? 0.55 : 0.42));
    }

    /** Classic two-tone coin with a feedback echo. */
    function coin(t) {
      const o = ctx.createOscillator(), g = ctx.createGain();
      o.type = "square";
      o.frequency.setValueAtTime(987.77, t); // B5
      o.frequency.setValueAtTime(1318.51, t + 0.075); // E6
      g.gain.setValueAtTime(0.0001, t);
      g.gain.exponentialRampToValueAtTime(0.3, t + 0.004);
      g.gain.setValueAtTime(0.3, t + 0.075);
      g.gain.exponentialRampToValueAtTime(0.0006, t + 0.5);
      const delay = ctx.createDelay(1), fb = ctx.createGain(), damp = ctx.createBiquadFilter();
      delay.delayTime.value = 0.17;
      fb.gain.value = 0.42;
      damp.type = "lowpass";
      damp.frequency.value = 3200;
      o.connect(g);
      g.connect(master);
      g.connect(delay);
      delay.connect(damp).connect(fb).connect(delay);
      damp.connect(master);
      o.start(t);
      o.stop(t + 0.55);
    }

    /** Filtered noise sweeping up and away. */
    function whoosh(t) {
      const len = Math.floor(ctx.sampleRate * 0.9), buf = ctx.createBuffer(1, len, ctx.sampleRate), d = buf.getChannelData(0);
      let seed = 1234567;
      for (let i = 0; i < len; i++) d[i] = ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2147483648) - 1;
      const src = ctx.createBufferSource(), bp = ctx.createBiquadFilter(), g = ctx.createGain();
      src.buffer = buf;
      bp.type = "bandpass";
      bp.Q.value = 0.8;
      bp.frequency.setValueAtTime(220, t);
      bp.frequency.exponentialRampToValueAtTime(2800, t + 0.3);
      bp.frequency.exponentialRampToValueAtTime(500, t + 0.75);
      g.gain.setValueAtTime(0.0001, t);
      g.gain.exponentialRampToValueAtTime(2.0, t + 0.26);
      g.gain.exponentialRampToValueAtTime(0.0006, t + 0.78);
      src.connect(bp).connect(g).connect(master);
      src.start(t);
      src.stop(t + 0.85);
    }
    return { scale, coin, whoosh, master };
  }

  /** Live player: null when sound is off or the system won't start audio without a click. */
  function start() {
    let on = true;
    try { on = localStorage.getItem("stray-intro-sound") !== "0"; } catch (e) {}
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!on || !AC) return null;
    let ctx;
    try { ctx = new AC(); } catch (e) { return null; }
    const s = build(ctx);
    const t0 = performance.now();
    let dead = false;
    const kill = () => {
      dead = true;
      ctx.close().catch(() => {});
    };
    const ready = ctx.state === "running" ? Promise.resolve() : ctx.resume();
    // If audio can't start right away, stay silent rather than play everything late.
    const timer = setTimeout(() => ctx.state !== "running" && kill(), 500);
    /** Play `what` so that it sounds `at` seconds after the animation began. */
    const cue = (what, at) =>
      ready.then(() => {
        if (dead || ctx.state !== "running") return;
        const late = (performance.now() - t0) / 1000 - at;
        if (late < 0.25) s[what](ctx.currentTime + Math.max(0.02, -late));
      }).catch(() => {});
    return {
      cue,
      now: (what) => !dead && ctx.state === "running" && s[what](ctx.currentTime + 0.02),
      /** Fade out what is still ringing and free the audio device. */
      end: (after = 1.2) => {
        clearTimeout(timer);
        setTimeout(kill, after * 1000);
      },
      silence: () => !dead && s.master.gain.setTargetAtTime(0, ctx.currentTime, 0.03),
    };
  }
  return { build, start };
})();
