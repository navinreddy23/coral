/**
 * An author's picture, shrunk to the size a node draws it.
 *
 * A canvas scales an image in one bilinear step, which samples only the pixels nearest each
 * output pixel: a 64-pixel picture drawn into an 18-pixel node skips most of it and comes out
 * jagged. Halving it first, as many times as it takes to come within twice the target, lets
 * every pixel count, and the last step is small enough for bilinear to be smooth.
 */
const shrunk = new WeakMap<object, Map<number, CanvasImageSource>>();

export function face(source: CanvasImageSource, px: number): CanvasImageSource {
  const size = Math.max(1, Math.round(px));
  if (typeof document === 'undefined' || !shrinkable(source) || source.width <= size * 2) {
    return source;
  }
  let sizes = shrunk.get(source);
  if (!sizes) {
    sizes = new Map();
    shrunk.set(source, sizes);
  }
  const held = sizes.get(size);
  if (held) return held;

  let step: CanvasImageSource = source;
  let w = source.width;
  let h = source.height;
  while (w > size * 2) {
    w = Math.ceil(w / 2);
    h = Math.ceil(h / 2);
    step = draw(step, w, h);
  }
  const done = draw(step, size, size);
  sizes.set(size, done);
  return done;
}

/** A decoded picture of known size: what the pictures arrive as, and what this makes. */
function shrinkable(source: CanvasImageSource): source is ImageBitmap | HTMLCanvasElement {
  return (
    (typeof ImageBitmap !== 'undefined' && source instanceof ImageBitmap) ||
    (typeof HTMLCanvasElement !== 'undefined' && source instanceof HTMLCanvasElement)
  );
}

function draw(from: CanvasImageSource, w: number, h: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  if (!ctx) return canvas;
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(from, 0, 0, w, h);
  return canvas;
}
