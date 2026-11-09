// Renders the StraySMP loading-screen logo for the Stray Client mod.
// Minecraft draws mojangstudios.png (512×512) as a wide 1024×256 logo: the top half is the
// left part, the bottom half the right part. Usage: node tools/gen_splash.js <out.png>
const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');
(async () => {
  const out = process.argv[2];
  const ui = path.resolve(__dirname, '../ui');
  const logo = fs.readFileSync(path.join(ui, 'assets/logo.svg'), 'utf8');
  const html = `<!doctype html><html><head><style>
    @font-face { font-family: U; src: url(file://${ui}/fonts/unbounded-latin-wght-normal.woff2); font-weight: 200 900; }
    html, body { margin: 0; background: transparent; }
    .wide { width: 1024px; height: 256px; display: flex; align-items: center; justify-content: center; gap: 26px; }
    .orb { width: 150px; height: 150px; image-rendering: pixelated; }
    .orb svg { width: 100%; height: 100%; }
    .txt { display: flex; flex-direction: column; align-items: flex-end; }
    .big { font: 800 104px/1 U; letter-spacing: .02em; color: #fff; }
    .small { font: 600 38px/1 U; letter-spacing: .32em; color: #51a2ff; margin-top: 8px; margin-right: -.32em; }
  </style></head><body><div class="wide" id="w"><div class="orb">${logo}</div>
  <div class="txt"><div class="big">STRAYSMP</div><div class="small">CLIENT</div></div></div></body></html>`;
  const tmp = path.join(require('os').tmpdir(), `splash-${process.pid}.html`);
  fs.writeFileSync(tmp, html);
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1024, height: 256 } });
  await p.goto('file://' + tmp);
  await p.waitForTimeout(500);
  const wide = await p.screenshot({ omitBackground: true });
  // Stack: left half on top, right half below → 512×512 like mojangstudios.png
  const p2 = await b.newPage({ viewport: { width: 512, height: 512 } });
  await p2.setContent(`<html><body style="margin:0;background:transparent">
    <div style="width:512px;height:256px;background:url(data:image/png;base64,${wide.toString('base64')}) 0 0/1024px 256px"></div>
    <div style="width:512px;height:256px;background:url(data:image/png;base64,${wide.toString('base64')}) -512px 0/1024px 256px"></div></body></html>`);
  await p2.waitForTimeout(200);
  await p2.screenshot({ path: out, omitBackground: true });
  await b.close();
  fs.unlinkSync(tmp);
  console.log('wrote', out);
})();
