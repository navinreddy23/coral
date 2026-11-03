/** The parts of the right panel that fold away to their heading. */
export type SectionKey = 'unstaged' | 'staged' | 'compose' | 'message' | 'files';

const KEYS: readonly SectionKey[] = ['unstaged', 'staged', 'compose', 'message', 'files'];

/**
 * How the right panel's height is shared out.
 *
 * `null` for a height that has never been dragged, which draws the section as tall as its
 * content wants. A shipped pixel height would be wrong for one commit message or the other,
 * and the panel looked right before any of this could be dragged.
 */
export interface SectionSizes {
  /** The unstaged list's part of the height the two lists share, so it survives a resize. */
  unstagedShare: number;
  composeHeight: number | null;
  messageHeight: number | null;
}

export type SizeKey = keyof SectionSizes;

/**
 * How far each height may be dragged.
 *
 * The floors keep every divider and heading in reach: a section dragged to nothing takes the
 * handle that would bring it back with it. The pixel maxima are only there to keep a stored
 * value sane; what the panel can actually spare is decided when it is drawn.
 */
export const SECTION_LIMITS: Record<SizeKey, { min: number; max: number }> = {
  unstagedShare: { min: 0.1, max: 0.9 },
  composeHeight: { min: 150, max: 1200 },
  messageHeight: { min: 60, max: 2400 },
};

/** The least an open file list is drawn at, heading included, before a split gives way. */
export const LIST_FLOOR_PX = 96;

const STORAGE_KEY = 'coral.sections';

export function clampSize(key: SizeKey, value: number): number {
  const { min, max } = SECTION_LIMITS[key];
  const clamped = Math.min(max, Math.max(min, value));
  return key === 'unstagedShare' ? clamped : Math.round(clamped);
}

/**
 * A dragged height as the panel can actually afford to draw it.
 *
 * The stored height knew nothing about the window: dragged tall in a maximised window, it
 * would leave the lists beside it nothing once the window was made smaller. `room` is the
 * height the section and its neighbours share and `floor` what the neighbours must keep.
 * The stored height is untouched, so making the window taller again puts it back.
 */
export function fitHeight(want: number, room: number, floor: number, min: number): number {
  if (room <= 0) return want;
  return Math.max(min, Math.min(want, room - floor));
}

/** Sizes and folds of the right panel's sections, remembered across launches. */
export class SectionsState {
  sizes = $state<SectionSizes>(defaultSizes());
  collapsed = $state<Record<SectionKey, boolean>>(defaultCollapsed());

  constructor() {
    const stored = read();
    if (stored) {
      this.sizes = stored.sizes;
      this.collapsed = stored.collapsed;
    }
  }

  resize(key: SizeKey, value: number): void {
    this.sizes = { ...this.sizes, [key]: clampSize(key, value) };
    this.save();
  }

  /** One split back as shipped, for the divider that was double-clicked. */
  reset(key: SizeKey): void {
    this.sizes = { ...this.sizes, [key]: defaultSizes()[key] };
    this.save();
  }

  toggle(key: SectionKey): void {
    this.collapsed = { ...this.collapsed, [key]: !this.collapsed[key] };
    this.save();
  }

  open(key: SectionKey): void {
    if (!this.collapsed[key]) return;
    this.toggle(key);
  }

  private save(): void {
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ ...this.sizes, collapsed: this.collapsed }),
      );
    } catch {
      // Losing the layout is not worth failing over.
    }
  }
}

function defaultSizes(): SectionSizes {
  return { unstagedShare: 0.5, composeHeight: null, messageHeight: null };
}

function defaultCollapsed(): Record<SectionKey, boolean> {
  return { unstaged: false, staged: false, compose: false, message: false, files: false };
}

/**
 * Reads the stored layout, clamping every size and keeping only the folds it recognises.
 *
 * A limit can change between releases, and a height stored under the old one would otherwise
 * come back as a section that cannot be dragged back into view.
 */
function read(): { sizes: SectionSizes; collapsed: Record<SectionKey, boolean> } | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return null;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) return null;
    const stored = parsed as Record<string, unknown>;

    const sizes = defaultSizes();
    for (const key of Object.keys(sizes) as SizeKey[]) {
      const value = stored[key];
      if (typeof value === 'number' && Number.isFinite(value)) {
        sizes[key] = clampSize(key, value);
      }
    }
    const collapsed = defaultCollapsed();
    const folds = stored['collapsed'];
    if (typeof folds === 'object' && folds !== null) {
      for (const key of KEYS) {
        if ((folds as Record<string, unknown>)[key] === true) collapsed[key] = true;
      }
    }
    return { sizes, collapsed };
  } catch {
    // A webview with storage disabled, or a corrupt entry, must still open the window.
    return null;
  }
}
