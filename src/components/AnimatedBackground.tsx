import { useEffect, useRef } from 'react';
import { BackgroundStyle } from '../types';

interface AnimatedBackgroundProps {
  disabled?: boolean;
  style?: BackgroundStyle;
  smokeIntensity?: number;
  blobCount?: number;
}

export function AnimatedBackground({
  disabled,
  style = 'embers',
  smokeIntensity = 5,
  blobCount = 5,
}: AnimatedBackgroundProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frameRef  = useRef<number>(0);

  useEffect(() => {
    if (disabled || style === 'none') return;

    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.imageSmoothingEnabled = false;

    let stop = false;
    let cleanup = () => {};

    const startRenderer = () => {
      stop = true;
      cancelAnimationFrame(frameRef.current);
      cleanup();
      stop = false;
      ctx.imageSmoothingEnabled = false;
      if (style === 'embers') {
        cleanup = runEmbers(canvas, ctx, smokeIntensity, blobCount, frameRef, () => stop);
      } else if (style === 'matrix') {
        cleanup = runMatrix(canvas, ctx, frameRef, () => stop);
      } else if (style === 'mc-terrain') {
        cleanup = runMcTerrain(canvas, ctx, frameRef, () => stop);
      } else if (style === 'night-sky') {
        cleanup = runNightSky(canvas, ctx, frameRef, () => stop);
      }
    };

    const resize = (force = false) => {
      const w = canvas.offsetWidth  || window.innerWidth;
      const h = canvas.offsetHeight || window.innerHeight;
      if (w < 1 || h < 1) return;
      if (!force && canvas.width === w && canvas.height === h) return;
      canvas.width  = w;
      canvas.height = h;
      startRenderer();
    };

    const ro = new ResizeObserver(() => resize(false));
    ro.observe(canvas);
    resize(true);

    const onVisibility = () => {
      if (document.hidden) {
        stop = true;
        cancelAnimationFrame(frameRef.current);
      } else {
        startRenderer();
      }
    };
    document.addEventListener('visibilitychange', onVisibility);

    return () => {
      stop = true;
      cancelAnimationFrame(frameRef.current);
      ro.disconnect();
      document.removeEventListener('visibilitychange', onVisibility);
      cleanup();
    };
  }, [disabled, style, smokeIntensity, blobCount]);

  return (
    <div className="app-background">
      <canvas
        ref={canvasRef}
        style={{
          position: 'absolute',
          inset: 0,
          width: '100%',
          height: '100%',
          pointerEvents: 'none',
          zIndex: 2,
          display: 'block',
        }}
      />
      <div className="haze-layer" />
    </div>
  );
}

// ─── Embers ───────────────────────────────────────────────────────────────────

interface Particle {
  x: number; y: number; vx: number; vy: number;
  size: number; opacity: number; targetOpacity: number;
  life: number; maxLife: number; kind: 'smoke' | 'blob';
}

function createParticle(w: number, h: number, kind: 'smoke' | 'blob'): Particle {
  if (kind === 'smoke') return {
    x: Math.random() * w, y: Math.random() * h,
    vx: (Math.random() - 0.5) * 4, vy: (Math.random() - 0.5) * 4,
    size: 60 + Math.random() * 80, opacity: 0.04 + Math.random() * 0.05,
    targetOpacity: 0.06, life: 0, maxLife: 25 + Math.random() * 35, kind,
  };
  return {
    x: Math.random() * w, y: Math.random() * h,
    vx: (Math.random() - 0.5) * 14, vy: (Math.random() - 0.5) * 14,
    size: 120 + Math.random() * 160, opacity: 0.08 + Math.random() * 0.08,
    targetOpacity: 0.12, life: 0, maxLife: 18 + Math.random() * 28, kind,
  };
}

function runEmbers(
  canvas: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D,
  smokeIntensity: number,
  blobCount: number,
  frameRef: React.MutableRefObject<number>,
  isStopped: () => boolean,
): () => void {
  const smokeCount  = Math.round((smokeIntensity / 10) * 80);
  const actualBlobs = Math.round((blobCount / 10) * 40);
  const mouse = { x: -1000, y: -1000 };

  const particles: Particle[] = [
    ...Array.from({ length: smokeCount }, () => createParticle(canvas.width, canvas.height, 'smoke')),
    ...Array.from({ length: actualBlobs }, () => createParticle(canvas.width, canvas.height, 'blob')),
  ];

  const onMouse = (e: MouseEvent) => { mouse.x = e.clientX; mouse.y = e.clientY; };
  window.addEventListener('mousemove', onMouse, { passive: true });

  let last = performance.now();

  const tick = (t: number) => {
    if (isStopped()) return;
    const dt = Math.min((t - last) / 1000, 0.05);
    last = t;
    const W = canvas.width, H = canvas.height;
    ctx.clearRect(0, 0, W, H);

    for (let i = 0; i < particles.length; i++) {
      const p = particles[i];
      if (p.kind === 'blob') {
        const dx = p.x - mouse.x, dy = p.y - mouse.y;
        const dist = Math.hypot(dx, dy);
        const R = 180;
        if (dist < R && dist > 0) {
          const f = (R - dist) / R, a = Math.atan2(dy, dx);
          p.vx += Math.cos(a) * f * 300 * dt;
          p.vy += Math.sin(a) * f * 300 * dt;
          p.targetOpacity = 0.35;
        } else { p.targetOpacity = 0.12; }
        p.vx *= 0.92; p.vy *= 0.92;
        const g = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, p.size);
        g.addColorStop(0,   `rgba(90,20,20,${p.opacity})`);
        g.addColorStop(0.4, `rgba(60,15,15,${p.opacity * 0.6})`);
        g.addColorStop(0.7, `rgba(40,10,10,${p.opacity * 0.3})`);
        g.addColorStop(1,   'rgba(20,5,5,0)');
        ctx.beginPath(); ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fillStyle = g; ctx.fill();
      } else {
        p.targetOpacity = 0.06; p.vx *= 0.97; p.vy *= 0.97;
        const g = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, p.size);
        g.addColorStop(0,   `rgba(30,30,30,${p.opacity})`);
        g.addColorStop(0.5, `rgba(20,20,20,${p.opacity * 0.5})`);
        g.addColorStop(1,   'rgba(10,10,10,0)');
        ctx.beginPath(); ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fillStyle = g; ctx.fill();
      }
      p.x += p.vx * dt + Math.sin(t * 0.0004 + i) * 0.4;
      p.y += p.vy * dt - 0.15 + Math.cos(t * 0.0002 + i * 0.3) * 0.2;
      p.opacity += (p.targetOpacity - p.opacity) * 0.04;
      p.life += dt;
      if (p.life > p.maxLife || p.y < -p.size) {
        const r = createParticle(W, H, p.kind);
        r.y = H + r.size; r.opacity = 0;
        particles[i] = r;
      }
    }
    frameRef.current = requestAnimationFrame(tick);
  };
  frameRef.current = requestAnimationFrame(tick);
  return () => window.removeEventListener('mousemove', onMouse);
}

// ─── Matrix ───────────────────────────────────────────────────────────────────

const MATRIX_CHARS = 'アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン0123456789ABCDEF';

function runMatrix(
  canvas: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D,
  frameRef: React.MutableRefObject<number>,
  isStopped: () => boolean,
): () => void {
  const FS = 14;
  const cols = Math.floor(canvas.width / FS);
  const columns = Array.from({ length: cols }, (_, i) => ({
    x: i * FS,
    y: Math.random() * -canvas.height,
    speed: 40 + Math.random() * 80,
    len: 10 + Math.floor(Math.random() * 20),
    chars: Array.from({ length: 30 }, () => MATRIX_CHARS[Math.floor(Math.random() * MATRIX_CHARS.length)]),
  }));
  let last = performance.now(), shuffleT = 0;
  const tick = (t: number) => {
    if (isStopped()) return;
    const dt = Math.min((t - last) / 1000, 0.05); last = t;
    shuffleT += dt;
    const W = canvas.width, H = canvas.height;
    ctx.clearRect(0, 0, W, H);
    ctx.font = `${FS}px monospace`;
    if (shuffleT > 0.08) {
      shuffleT = 0;
      for (const c of columns) {
        c.chars[Math.floor(Math.random() * c.chars.length)] =
          MATRIX_CHARS[Math.floor(Math.random() * MATRIX_CHARS.length)];
      }
    }
    for (const c of columns) {
      c.y += c.speed * dt;
      for (let j = 0; j < c.len; j++) {
        const cy = c.y - j * FS;
        if (cy < 0 || cy > H) continue;
        const alpha = j === 0 ? 1 : (1 - j / c.len) * 0.6;
        ctx.fillStyle = j === 0 ? `rgba(180,255,180,${alpha})` : `rgba(0,180,60,${alpha})`;
        const ci = (Math.floor(c.y / FS) - j + c.chars.length) % c.chars.length;
        ctx.fillText(c.chars[ci], c.x, cy);
      }
      if (c.y - c.len * FS > H) {
        c.y = Math.random() * -H * 0.5;
        c.speed = 40 + Math.random() * 80;
        c.len = 10 + Math.floor(Math.random() * 20);
      }
    }
    frameRef.current = requestAnimationFrame(tick);
  };
  frameRef.current = requestAnimationFrame(tick);
  return () => {};
}

// ─── Minecraft shared helpers ─────────────────────────────────────────────────

const PX = 8; // block pixel size — large enough to clearly see

// Sky gradient colours
const SKY_TOP    = '#7ec8e3'; // daytime light blue
const SKY_MID    = '#4a9aba';
const SKY_HOR    = '#8bc34a'; // horizon green tint

// Block colours
const GRASS_TOP   = '#5d9e2f';
const GRASS_SIDE  = '#48851f';
const DIRT_A      = '#8b5e2a';
const DIRT_B      = '#7a5020';
const STONE_A     = '#7a7a7a';
const STONE_B     = '#5c5c5c';
const DEEP        = '#3a3a3a';
const TRUNK       = '#6b4423';
const LEAF_A      = '#3a7d1e';
const LEAF_B      = '#2d6218';
const LEAF_BG_A   = '#2d6218';
const LEAF_BG_B   = '#235010';

function px(ctx: CanvasRenderingContext2D, x: number, y: number, color: string) {
  ctx.fillStyle = color;
  ctx.fillRect(Math.round(x / PX) * PX, Math.round(y / PX) * PX, PX, PX);
}

function buildHeights(numCols: number, baseY: number, amp: number, seed: number): number[] {
  return Array.from({ length: numCols }, (_, i) => {
    const x = i * PX + seed;
    const n = Math.sin(x * 0.006) * amp
            + Math.sin(x * 0.018) * amp * 0.4
            + Math.sin(x * 0.05)  * amp * 0.15;
    return Math.round((baseY + n) / PX) * PX;
  });
}

interface Tree { col: number; trunkH: number; canopyR: number }

function buildTrees(numCols: number): Tree[] {
  const trees: Tree[] = [];
  let last = -20;
  for (let c = 6; c < numCols - 6; c++) {
    if (c - last < 10 + Math.floor(Math.random() * 8)) continue;
    if (Math.random() < 0.13) {
      trees.push({ col: c, trunkH: 3 + Math.floor(Math.random() * 3), canopyR: 2 + Math.floor(Math.random() * 2) });
      last = c;
    }
  }
  return trees;
}

function drawLayer(
  ctx: CanvasRenderingContext2D,
  W: number, H: number,
  heights: number[], trees: Tree[],
  scrollPx: number, isFg: boolean,
) {
  const numCols = heights.length;
  const colOff  = Math.floor(scrollPx / PX);
  const subPx   = scrollPx % PX;
  const visCols = Math.ceil(W / PX) + 2;

  for (let ci = 0; ci < visCols; ci++) {
    const src = ((ci + colOff) % numCols + numCols) % numCols;
    const sy  = heights[src];
    const dx  = ci * PX - subPx;

    // grass top + side
    px(ctx, dx, sy,      isFg ? GRASS_TOP  : LEAF_BG_A);
    px(ctx, dx, sy + PX, isFg ? GRASS_SIDE : LEAF_BG_B);

    // dirt band
    const dirtRows = isFg ? 6 : 3;
    for (let d = 2; d < dirtRows + 2; d++)
      px(ctx, dx, sy + d * PX, d % 2 === 0 ? DIRT_A : DIRT_B);

    // stone band
    const stoneStart = sy + (dirtRows + 2) * PX;
    const stoneRows  = Math.ceil(H * 0.14 / PX);
    for (let s = 0; s < stoneRows; s++)
      px(ctx, dx, stoneStart + s * PX, s % 2 === 0 ? STONE_A : STONE_B);

    // deep fill — solid rect is faster
    const deepStart = stoneStart + stoneRows * PX;
    ctx.fillStyle = DEEP;
    ctx.fillRect(Math.round(dx / PX) * PX, deepStart, PX, H - deepStart);
  }

  // trees
  for (const tree of trees) {
    const drawCol = tree.col - colOff;
    if (drawCol < -tree.canopyR - 2 || drawCol > visCols + tree.canopyR + 2) continue;
    const src = ((tree.col) % numCols + numCols) % numCols;
    const sy  = heights[src];
    const dx  = drawCol * PX - subPx;
    const tA  = isFg ? TRUNK  : LEAF_BG_A;
    const lA  = isFg ? LEAF_A : LEAF_BG_A;
    const lB  = isFg ? LEAF_B : LEAF_BG_B;

    for (let t = 1; t <= tree.trunkH; t++)
      px(ctx, dx, sy - t * PX, tA);

    const base = sy - tree.trunkH * PX;
    const r = tree.canopyR;
    for (let dy2 = -r; dy2 <= 1; dy2++)
      for (let dx2 = -r; dx2 <= r; dx2++) {
        if (Math.abs(dx2) === r && Math.abs(dy2) === r) continue;
        px(ctx, dx + dx2 * PX, base + dy2 * PX, (dx2 + dy2) % 2 === 0 ? lA : lB);
      }
  }
}

// ─── MC Terrain ───────────────────────────────────────────────────────────────

// Cloud colours — Minecraft white/light-grey pixel clouds
const CLOUD_A = '#f0f0f0';
const CLOUD_B = '#d8d8d8';

interface Cloud {
  x: number;   // leading-edge x in pixels (can be fractional)
  y: number;   // top-left row in pixels (snapped to PX grid)
  w: number;   // width in blocks
  h: number;   // height in blocks
  speed: number; // px/s
}

function buildClouds(W: number, H: number): Cloud[] {
  const clouds: Cloud[] = [];
  const skyH = H * 0.45; // clouds live in top 45% of sky
  let x = PX * 4;
  while (x < W + W * 0.5) {
    const w = 6 + Math.floor(Math.random() * 8);   // 6–13 blocks wide
    const h = 2 + Math.floor(Math.random() < 0.4 ? 1 : 0); // 2 or 3 blocks tall
    const y = Math.round((PX * 3 + Math.random() * skyH) / PX) * PX;
    clouds.push({ x, y, w, h, speed: 12 + Math.random() * 18 });
    x += (w + 6 + Math.floor(Math.random() * 12)) * PX;
  }
  return clouds;
}

function drawCloud(ctx: CanvasRenderingContext2D, cloud: Cloud) {
  const cx = Math.round(cloud.x / PX) * PX;
  const cy = cloud.y;
  const w  = cloud.w;
  const h  = cloud.h;
  for (let row = 0; row < h; row++) {
    for (let col = 0; col < w; col++) {
      // Indent corners for a rounded look
      if ((row === 0 || row === h - 1) && (col === 0 || col === w - 1)) continue;
      const colour = (row === 0 || col === 0) ? CLOUD_B : CLOUD_A;
      ctx.fillStyle = colour;
      ctx.fillRect(cx + col * PX, cy + row * PX, PX, PX);
    }
  }
}

function runMcTerrain(
  canvas: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D,
  frameRef: React.MutableRefObject<number>,
  isStopped: () => boolean,
): () => void {
  const W = canvas.width, H = canvas.height;

  const EXTRA = 160; // extra off-screen cols for seamless wrap
  const numCols = Math.ceil(W / PX) + EXTRA;

  const fgHeights = buildHeights(numCols, H * 0.70, H * 0.09, 0);
  const bgHeights = buildHeights(numCols, H * 0.55, H * 0.06, 1300);
  const fgTrees   = buildTrees(numCols);
  const bgTrees   = buildTrees(numCols);

  const clouds = buildClouds(W, H);

  let fgScroll = 0, bgScroll = 0;
  let last = performance.now();

  const tick = (t: number) => {
    if (isStopped()) return;
    const dt = Math.min((t - last) / 1000, 0.05);
    last = t;
    const W2 = canvas.width, H2 = canvas.height;

    // Sky gradient
    const sky = ctx.createLinearGradient(0, 0, 0, H2);
    sky.addColorStop(0,    SKY_TOP);
    sky.addColorStop(0.55, SKY_MID);
    sky.addColorStop(1,    SKY_HOR);
    ctx.fillStyle = sky;
    ctx.fillRect(0, 0, W2, H2);

    // Clouds — scroll and wrap
    for (const c of clouds) {
      c.x -= c.speed * dt;
      if (c.x + c.w * PX < 0) c.x = W2 + Math.random() * PX * 16;
      drawCloud(ctx, c);
    }

    // Background layer (dimmed)
    ctx.globalAlpha = 0.45;
    bgScroll = (bgScroll + 12 * dt) % (numCols * PX);
    drawLayer(ctx, W2, H2, bgHeights, bgTrees, bgScroll, false);
    ctx.globalAlpha = 1;

    // Foreground layer
    fgScroll = (fgScroll + 28 * dt) % (numCols * PX);
    drawLayer(ctx, W2, H2, fgHeights, fgTrees, fgScroll, true);

    frameRef.current = requestAnimationFrame(tick);
  };

  frameRef.current = requestAnimationFrame(tick);
  return () => {};
}

// ─── Night Sky ────────────────────────────────────────────────────────────────

// Night sky colours
const NIGHT_TOP   = '#060d1f';
const NIGHT_MID   = '#0a1628';
const NIGHT_HOR   = '#0d1f36';

interface Star {
  x: number; y: number;
  phase: number; speed: number;
  bright: number;
}

interface ShootingStar {
  x: number; y: number;
  vx: number; vy: number;
  len: number;
  life: number; maxLife: number;
  active: boolean;
}

interface Firefly {
  x: number; y: number;
  vx: number; vy: number;
  phase: number;
  driftT: number; driftInterval: number;
  targetVx: number; targetVy: number;
}

function runNightSky(
  canvas: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D,
  frameRef: React.MutableRefObject<number>,
  isStopped: () => boolean,
): () => void {
  const W = canvas.width, H = canvas.height;

  const STAR_COUNT = 120;
  const stars: Star[] = Array.from({ length: STAR_COUNT }, () => ({
    x: Math.floor(Math.random() * Math.ceil(W / PX)) * PX,
    y: Math.floor(Math.random() * Math.ceil(H / PX)) * PX,
    phase: Math.random() * Math.PI * 2,
    speed: 0.5 + Math.random() * 1.5,
    bright: 0.4 + Math.random() * 0.6,
  }));

  const shoots: ShootingStar[] = Array.from({ length: 3 }, () => ({
    x: 0, y: 0, vx: 0, vy: 0, len: 0,
    life: 0, maxLife: 0, active: false,
  }));
  let nextShoot = 3 + Math.random() * 6;

  const FIREFLY_COUNT = 20;
  const fireflies: Firefly[] = Array.from({ length: FIREFLY_COUNT }, () => {
    const tvx = (Math.random() - 0.5) * 18;
    const tvy = (Math.random() - 0.5) * 18;
    return {
      x: Math.random() * W,
      y: Math.random() * H,
      vx: tvx, vy: tvy,
      phase: Math.random() * Math.PI * 2,
      driftT: 0,
      driftInterval: 1.5 + Math.random() * 2.5,
      targetVx: tvx, targetVy: tvy,
    };
  });

  let last = performance.now();

  const tick = (t: number) => {
    if (isStopped()) return;
    const dt = Math.min((t - last) / 1000, 0.05);
    last = t;
    const W2 = canvas.width, H2 = canvas.height;

    // Night sky gradient
    const sky = ctx.createLinearGradient(0, 0, 0, H2);
    sky.addColorStop(0,   NIGHT_TOP);
    sky.addColorStop(0.6, NIGHT_MID);
    sky.addColorStop(1,   NIGHT_HOR);
    ctx.fillStyle = sky;
    ctx.fillRect(0, 0, W2, H2);

    // Stars
    for (const s of stars) {
      s.phase += dt * s.speed;
      const twinkle = (Math.sin(s.phase) * 0.5 + 0.5) * s.bright;
      if (twinkle < 0.05) continue;
      ctx.fillStyle = `rgba(220,230,255,${twinkle})`;
      ctx.fillRect(s.x, s.y, PX, PX);
    }

    // Shooting stars
    nextShoot -= dt;
    if (nextShoot <= 0) {
      const slot = shoots.find(s => !s.active);
      if (slot) {
        slot.x = Math.random() * W2 * 0.8;
        slot.y = Math.random() * H2 * 0.35;
        const angle = Math.PI * 0.18 + Math.random() * 0.2;
        const speed = 300 + Math.random() * 200;
        slot.vx = Math.cos(angle) * speed;
        slot.vy = Math.sin(angle) * speed;
        slot.len = 60 + Math.random() * 80;
        slot.life = 0;
        slot.maxLife = slot.len / speed + 0.3 + Math.random() * 0.2;
        slot.active = true;
      }
      nextShoot = 4 + Math.random() * 8;
    }

    for (const s of shoots) {
      if (!s.active) continue;
      s.life += dt;
      const progress = s.life / s.maxLife;
      if (progress >= 1) { s.active = false; continue; }

      const tailAlpha = Math.max(0, 1 - progress) * 0.9;
      const tx = s.x + s.vx * s.life;
      const ty = s.y + s.vy * s.life;
      const tailX = tx - (s.vx / Math.hypot(s.vx, s.vy)) * s.len * (1 - progress);
      const tailY = ty - (s.vy / Math.hypot(s.vx, s.vy)) * s.len * (1 - progress);

      const grad = ctx.createLinearGradient(tailX, tailY, tx, ty);
      grad.addColorStop(0, `rgba(200,220,255,0)`);
      grad.addColorStop(1, `rgba(220,235,255,${tailAlpha})`);
      ctx.strokeStyle = grad;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(tailX, tailY);
      ctx.lineTo(tx, ty);
      ctx.stroke();

      ctx.fillStyle = `rgba(240,245,255,${tailAlpha})`;
      ctx.fillRect(Math.round(tx / PX) * PX, Math.round(ty / PX) * PX, PX, PX);
    }

    // Fireflies
    for (const f of fireflies) {
      f.phase += dt * (0.8 + Math.random() * 0.4);
      f.driftT += dt;
      if (f.driftT >= f.driftInterval) {
        f.driftT = 0;
        f.driftInterval = 1.5 + Math.random() * 2.5;
        f.targetVx = (Math.random() - 0.5) * 18;
        f.targetVy = (Math.random() - 0.5) * 18;
      }
      f.vx += (f.targetVx - f.vx) * dt * 1.2;
      f.vy += (f.targetVy - f.vy) * dt * 1.2;
      f.x += f.vx * dt;
      f.y += f.vy * dt;
      if (f.x < 0) { f.x = 0; f.targetVx = Math.abs(f.targetVx); }
      if (f.x > W2) { f.x = W2; f.targetVx = -Math.abs(f.targetVx); }
      if (f.y < 0) { f.y = 0; f.targetVy = Math.abs(f.targetVy); }
      if (f.y > H2) { f.y = H2; f.targetVy = -Math.abs(f.targetVy); }

      const glow = (Math.sin(f.phase) * 0.5 + 0.5);
      const alpha = 0.25 + glow * 0.65;
      const cx = Math.round(f.x / PX) * PX;
      const cy = Math.round(f.y / PX) * PX;

      const halo = ctx.createRadialGradient(cx + PX / 2, cy + PX / 2, 0, cx + PX / 2, cy + PX / 2, PX * 3);
      halo.addColorStop(0,   `rgba(255,240,80,${alpha * 0.45})`);
      halo.addColorStop(0.5, `rgba(255,210,40,${alpha * 0.15})`);
      halo.addColorStop(1,   'rgba(200,160,0,0)');
      ctx.fillStyle = halo;
      ctx.beginPath();
      ctx.arc(cx + PX / 2, cy + PX / 2, PX * 3, 0, Math.PI * 2);
      ctx.fill();

      ctx.fillStyle = `rgba(255,245,120,${alpha})`;
      ctx.fillRect(cx, cy, PX, PX);
    }

    frameRef.current = requestAnimationFrame(tick);
  };

  frameRef.current = requestAnimationFrame(tick);
  return () => {};
}
