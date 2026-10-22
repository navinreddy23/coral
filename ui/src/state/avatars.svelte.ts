import { authorAvatar } from '../ipc/avatar';

const STORAGE_KEY = 'coral.avatars';

/** One author's picture: what the canvas draws, and what an image element shows. */
interface Picture {
  bitmap: ImageBitmap;
  url: string;
}

/**
 * Authors' pictures, from GitHub for its noreply addresses and from Gravatar for the rest.
 *
 * On unless turned off in Appearance, which says where they come from: asking for one sends
 * Gravatar a hash of the author's address. Each address is asked about once per launch, and
 * the engine keeps what it learns on disk, so scrolling back over a history asks nobody again.
 */
export class AvatarState {
  enabled = $state(true);
  /** Bumped as pictures arrive, so the canvas, which holds no reactive state, repaints. */
  version = $state(0);

  #pictures = new Map<string, Picture | null>();
  #asked = new Set<string>();

  constructor() {
    try {
      this.enabled = localStorage.getItem(STORAGE_KEY) !== 'false';
    } catch {
      this.enabled = true;
    }
  }

  setEnabled(on: boolean): void {
    this.enabled = on;
    try {
      localStorage.setItem(STORAGE_KEY, String(on));
    } catch {
      // Remembered for this launch only.
    }
    this.version += 1;
  }

  /** Asks for the pictures of these addresses, once each. */
  want(emails: Iterable<string>): void {
    if (!this.enabled) return;
    for (const raw of emails) {
      const email = raw.trim().toLowerCase();
      if (email === '' || this.#asked.has(email)) continue;
      this.#asked.add(email);
      void this.#fetch(email);
    }
  }

  async #fetch(email: string): Promise<void> {
    let picture: Picture | null = null;
    try {
      const bytes = await authorAvatar(email);
      if (bytes !== null) {
        const blob = new Blob([bytes]);
        picture = { bitmap: await createImageBitmap(blob), url: URL.createObjectURL(blob) };
      }
    } catch {
      // Bytes that do not decode as a picture are no picture; the initials stay.
      picture = null;
    }
    this.#pictures.set(email, picture);
    if (picture !== null) this.version += 1;
  }

  /** What the canvas draws for an author, or null to draw their initials. */
  bitmap(email: string | undefined): ImageBitmap | null {
    if (!this.enabled || email === undefined) return null;
    return this.#pictures.get(email.trim().toLowerCase())?.bitmap ?? null;
  }

  /** The same, for an image element. */
  url(email: string | undefined): string | null {
    if (!this.enabled || email === undefined) return null;
    return this.#pictures.get(email.trim().toLowerCase())?.url ?? null;
  }
}
