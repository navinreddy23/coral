export type Theme = 'light' | 'dark';

/** What the window was told to do about its theme, as opposed to what it is showing. */
export type ThemeChoice = Theme | 'system';

const STORAGE_KEY = 'coral.theme';
const QUERY = '(prefers-color-scheme: dark)';

/**
 * The interface theme.
 *
 * It follows the desktop unless it has been told otherwise, and it keeps following it: a
 * desktop that switches to dark in the evening takes Coral with it without the window being
 * restarted.
 *
 * The stored value used to be a theme and is now a choice, and the old values still mean what
 * they meant. Nothing ever wrote `light` or `dark` there except somebody pressing the switch,
 * so a stored theme is an explicit choice and is honoured; an empty store is somebody who
 * never expressed one, which is what `system` is for.
 */
export class ThemeState {
  /** What was asked for. */
  choice = $state<ThemeChoice>('system');

  /** What the desktop is asking for, watched rather than read once. */
  #system = $state<Theme>('light');

  /** Undoes the listener. Held so a test can take it back. */
  #stopWatching: (() => void) | null = null;

  /** What is actually on screen, which is the only thing anything else needs. */
  get current(): Theme {
    return this.choice === 'system' ? this.#system : this.choice;
  }

  /** Whether the desktop is deciding, for a control that has to say so. */
  get following(): boolean {
    return this.choice === 'system';
  }

  constructor() {
    this.#system = ask();
    this.choice = read() ?? 'system';
    this.#watch();
    this.apply();
  }

  /**
   * Between light and dark, which is what the switch in the title bar does.
   *
   * Pressing it is an explicit choice, so it stops following the desktop. Somebody who wants
   * that back asks for it in Preferences, where the three states can be shown; a switch that
   * cycles through three is a switch nobody can predict.
   */
  toggle(): void {
    this.set(this.current === 'light' ? 'dark' : 'light');
  }

  set(choice: ThemeChoice): void {
    this.choice = choice;
    this.apply();
    write(choice);
  }

  /** Stops watching the desktop. For a test, and for a component that is going away. */
  release(): void {
    this.#stopWatching?.();
    this.#stopWatching = null;
  }

  #watch(): void {
    const media = matchMediaOrNull();
    if (!media) return;
    const changed = (event: { matches: boolean }) => {
      this.#system = event.matches ? 'dark' : 'light';
      this.apply();
    };
    media.addEventListener('change', changed);
    this.#stopWatching = () => media.removeEventListener('change', changed);
  }

  private apply(): void {
    const root = document.documentElement;
    // Light is the absence of the attribute, so the bare `:root` block in tokens.css holds
    // the whole light palette and dark is the only thing that has to be written down twice.
    if (this.current === 'dark') root.setAttribute('data-theme', 'dark');
    else root.removeAttribute('data-theme');
  }
}

/** What the desktop prefers right now, or light where nothing can be asked. */
function ask(): Theme {
  return matchMediaOrNull()?.matches ? 'dark' : 'light';
}

/**
 * The media query, or nothing.
 *
 * A webview old enough to lack `matchMedia`, and every test that stubs `window` away, has to
 * leave the window openable rather than throwing on the way up.
 */
function matchMediaOrNull(): MediaQueryList | null {
  try {
    return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
      ? window.matchMedia(QUERY)
      : null;
  } catch {
    return null;
  }
}

function read(): ThemeChoice | null {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    return value === 'dark' || value === 'light' || value === 'system' ? value : null;
  } catch {
    // A webview with storage disabled must still open.
    return null;
  }
}

function write(choice: ThemeChoice): void {
  try {
    localStorage.setItem(STORAGE_KEY, choice);
  } catch {
    // Losing the preference is not worth failing over.
  }
}
