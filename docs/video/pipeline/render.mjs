// Deterministic frame renderer for the intro video.
//
// For each scene page built by build_scenes.py, this steps
// window.__setTime(t) at exactly 30 fps, screenshots every
// state, and encodes the frames to a per-scene MP4 segment
// with ffmpeg. No wall-clock animation is involved: a scene
// renders identically on every run.
//
// Usage:
//   node docs/video/pipeline/render.mjs [--only scene3,hero]
//                                       [--keep-frames]
// Playwright is resolved from web/node_modules explicitly,
// so the script runs from any working directory.
import { execFileSync } from 'node:child_process';
import { mkdirSync, rmSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const require = createRequire(path.join(REPO, 'web', 'package.json'));
const { chromium } = require('playwright');

const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf('--' + name);
  return i >= 0 ? args[i + 1] : dflt;
};
const WORK = opt('work', path.join(os.homedir(), 'workspace/.video-work'));
const FPS = 30;
const SCENES = {
  scene1: 10, scene2: 10, scene3: 10,
  scene4: 10, scene5: 10, scene6: 10,
  hero: 8,
};
const only = opt('only', Object.keys(SCENES).join(',')).split(',');
const keepFrames = args.includes('--keep-frames');

function ffmpeg(argv) {
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', ...argv], { stdio: 'inherit' });
}

// Crop the /device screenshots to the card region once.
const shots = path.join(WORK, 'shots');
const crop = path.join(shots, 'device-crop.png');
if (!existsSync(crop)) {
  ffmpeg(['-i', path.join(shots, 'device-approve.png'),
    '-vf', 'crop=600:430:780:55', crop]);
  console.log('made device-crop.png');
}

mkdirSync(path.join(WORK, 'segments'), { recursive: true });
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
page.on('pageerror', (e) => console.log('pageerror (logged, continuing):', e.message));
const failed = [];

for (const name of only) {
  const seconds = SCENES[name];
  if (!seconds) throw new Error('unknown scene: ' + name);
  try {
    const framesDir = path.join(WORK, 'frames', name);
    rmSync(framesDir, { recursive: true, force: true });
    mkdirSync(framesDir, { recursive: true });
    await page.goto('file://' + path.join(WORK, 'scenes', name + '.html'));
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(500);
    const total = seconds * FPS;
    // JPEG frames: PNG encoding a photographic 1080p frame
    // costs ~1.5 s in software rendering here; q92 JPEG is
    // ~0.2 s and the segments are re-encoded to H.264/VP9
    // immediately after, so nothing downstream can tell.
    for (let f = 0; f < total; f++) {
      const t = f / FPS;
      await page.evaluate((tt) => window.__setTime(tt), t);
      await page.screenshot({
        path: path.join(framesDir, 'frame_' + String(f).padStart(4, '0') + '.jpg'),
        type: 'jpeg',
        quality: 92,
      });
      if (f % 60 === 59) console.log(name, 'frame', f + 1, '/', total);
    }
    const segOut = path.join(WORK, 'segments', name + '.mp4');
    ffmpeg([
      '-framerate', String(FPS), '-i', path.join(framesDir, 'frame_%04d.jpg'),
      '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18',
      '-preset', 'medium', '-r', String(FPS),
      '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709',
      segOut,
    ]);
    console.log('segment done:', name, total, 'frames');
    if (!keepFrames) rmSync(framesDir, { recursive: true, force: true });
  } catch (err) {
    console.error('SCENE FAILED:', name, err.message);
    failed.push(name);
  }
}

// Poster: scene 3 at the card hold (t = 6.8 s). PNG here -
// the poster is a still, so the slow encode is paid once.
await page.goto('file://' + path.join(WORK, 'scenes', 'scene3.html'));
await page.evaluate(() => document.fonts.ready);
await page.waitForTimeout(400);
await page.evaluate(() => window.__setTime(6.8));
await page.screenshot({ path: path.join(WORK, 'poster-src.png') });
console.log('poster source rendered');
await browser.close();
if (failed.length) {
  console.error('FAILED SCENES:', failed.join(', '));
  process.exit(1);
}
