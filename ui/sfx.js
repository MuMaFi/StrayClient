// Start-up sounds, synthesised with the Web Audio API (no audio files):
//  • a warm e-piano running up a Cmaj9 arpeggio while the logo builds, ending on the chord,
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

    // Warm bus for the e-piano: no bit crusher, a soft lowpass and a short room echo.
    const warm = ctx.createBiquadFilter();
    warm.type = "lowpass";
    warm.frequency.value = 2600;
    const room = ctx.createDelay(1), roomFb = ctx.createGain(), roomLp = ctx.createBiquadFilter();
    room.delayTime.value = 0.11;
    roomFb.gain.value = 0.28;
    roomLp.type = "lowpass";
    roomLp.frequency.value = 1800;
    warm.connect(master);
    warm.connect(room);
    room.connect(roomLp).connect(roomFb).connect(room);
    roomLp.connect(master);

    // Rhodes-style e-piano: two slightly detuned sine carriers with a gentle FM "tine",
    // a soft attack and a long natural decay.
    function piano(freq, t, len = 1.2, vol = 0.4) {
      const amp = ctx.createGain();
      amp.gain.setValueAtTime(0.0001, t);
      amp.gain.exponentialRampToValueAtTime(vol, t + 0.012);
      amp.gain.exponentialRampToValueAtTime(vol * 0.45, t + 0.25);
      amp.gain.exponentialRampToValueAtTime(0.0006, t + len);
      amp.connect(warm);
      const mod = ctx.createOscillator(), idx = ctx.createGain();
      mod.frequency.value = freq; // 1:1 ratio = mellow tine
      idx.gain.setValueAtTime(freq * 0.9, t);
      idx.gain.exponentialRampToValueAtTime(freq * 0.05, t + 0.35);
      mod.connect(idx);
      const oscs = [mod];
      for (const det of [-3, 3]) {
        const car = ctx.createOscillator(), g = ctx.createGain();
        car.frequency.value = freq;
        car.detune.value = det;
        g.gain.value = 0.5;
        idx.connect(car.frequency);
        car.connect(g).connect(amp);
        oscs.push(car);
      }
      for (const o of oscs) {
        o.start(t);
        o.stop(t + len + 0.05);
      }
    }
    // Rising Cmaj9 arpeggio (C3 G3 B3 D4 E4 G4 B4 D5): jazzy, open, bright, not sad.
    const ARP = [130.81, 196.0, 246.94, 293.66, 329.63, 392.0, 493.88, 587.33];
    const ARP_STEP = 0.13;
    /** The arpeggio, then a soft Cmaj9 chord that rings under the coin. */
    function scale(t, step = ARP_STEP) {
      ARP.forEach((f, i) => piano(f, t + i * step, i === ARP.length - 1 ? 2.2 : 1.3, 0.34 + i * 0.012));
      const end = t + ARP.length * step;
      for (const f of [130.81, 246.94, 329.63, 392.0]) piano(f, end, 2.4, 0.16);
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
    return { scale, coin, whoosh, master, arpLength: ARP.length * ARP_STEP };
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
