export type Theme = 'light' | 'dark';

const STORAGE_KEY = 'coral.theme';

/**
 * The interface theme.
 *
 * Light is the default. The choice is explicit rather than following the system, so the
 * preference is stable across machines with different system settings; the stored value is
 * the only thing that changes it.
 */
export class ThemeState {
  current = $state<Theme>('light');

  constructor() {
    this.current = read() ?? 'light';
    this.apply();
  }

  toggle(): void {
    this.current = this.current === 'light' ? 'dark' : 'light';
    this.apply();
    write(this.current);
  }

  private apply(): void {
    const root = document.documentElement;
    if (this.current === 'dark') root.setAttribute('data-theme', 'dark');
    else root.removeAttribute('data-theme');
  }
}

function read(): Theme | null {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return v === 'dark' || v === 'light' ? v : null;
  } catch {
    // A webview with storage disabled must still open.
    return null;
  }
}

function write(theme: Theme): void {
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Losing the preference is not worth failing over.
  }
}
