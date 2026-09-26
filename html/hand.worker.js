// 手势识别 Worker：MediaPipe 在这里跑，主线程只管 3D 渲染和界面，互不抢时间。
// 主线程每来一帧新画面送一张 ImageBitmap，这里识别完只回传 21 个关键点。
import { FilesetResolver, HandLandmarker } from './vendor/mediapipe/vision_bundle.js';

// tasks-vision 在 module worker 里加载 wasm 胶水时走 self.import，并期望随后 self.ModuleFactory 已就位
self.import = async (url) => {
  const mod = await import(url);
  self.ModuleFactory = mod.default;
  return mod;
};
// 胶水代码在严格模式下会引用这个全局
self.custom_dbg = (...args) => console.debug(...args);

let landmarker = null;

async function init(wasm, model) {
  const vision = await FilesetResolver.forVisionTasks(wasm, true);
  const options = delegate => ({
    baseOptions: { modelAssetPath: model, delegate },
    runningMode: 'VIDEO', numHands: 1,
    minHandDetectionConfidence: 0.6, minHandPresenceConfidence: 0.6, minTrackingConfidence: 0.5
  });
  try { landmarker = await HandLandmarker.createFromOptions(vision, options('GPU')); return 'GPU'; }
  catch { landmarker = await HandLandmarker.createFromOptions(vision, options('CPU')); return 'CPU'; }
}

self.onmessage = async (event) => {
  const msg = event.data;
  if (msg.type === 'init') {
    try { const delegate = await init(msg.wasm, msg.model); self.postMessage({ type: 'ready', delegate }); }
    catch (err) { self.postMessage({ type: 'error', message: String(err?.message || err) }); }
    return;
  }
  if (msg.type === 'frame') {
    const { bitmap, t } = msg;
    let points = null;
    try {
      const lm = landmarker?.detectForVideo(bitmap, t)?.landmarks?.[0];
      if (lm) {
        points = new Float32Array(lm.length * 2);
        lm.forEach((p, i) => { points[i * 2] = p.x; points[i * 2 + 1] = p.y; });
      }
    } catch {}
    bitmap.close();
    self.postMessage({ type: 'result', t, points }, points ? [points.buffer] : []);
  }
};
