// Keyboard shortcuts: what a binding is, how a key press is named and matched, and when a key
// belongs to a field instead. The bindings themselves are in keymap.ts.

export interface Shortcut {
  /** Key combinations, e.g. 'G', 'Shift+Z', 'Ctrl+K', '?', 'ArrowLeft'. */
  keys: string[];
  label: string;
  group: string;
  /** Live only while this holds. */
  when?: () => boolean;
  /** Return false to let the key go on to the next binding. */
  run: (e: KeyboardEvent) => void | boolean;
  /** Also while typing in a field. */
  inInputs?: boolean;
  /** Held down, it repeats. */
  repeat?: boolean;
  /** Left out of the shortcuts list (another entry describes it). */
  hidden?: boolean;
}

export const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);

/** Keys a focused slider, checkbox or list keeps for itself. */
const CONTROL_KEYS = new Set(['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown', ' ', 'Enter']);

/** A key press as a combination: modifiers (Ctrl for Ctrl or ⌘) then the key; letters in
 * capitals; Shift only with keys that don't change with it (letters, named keys). */
export function comboOf(e: KeyboardEvent): string {
  let k = e.key;
  // Digits and the backquote by position, so other layouts get them too.
  if (/^Digit\d$/.test(e.code) && !e.shiftKey) k = e.code.slice(5);
  else if (e.code === 'Backquote' && !e.shiftKey) k = '`';
  if (k.length === 1) k = k.toUpperCase();
  const symbol = k.length === 1 && !/[A-Z]/.test(k);
  const mods: string[] = [];
  if (e.ctrlKey || e.metaKey) mods.push('Ctrl');
  if (e.shiftKey && !symbol) mods.push('Shift');
  return [...mods, k === ' ' ? 'Space' : k].join('+');
}

/** What has the key: a text field, a control (slider, checkbox, list), a button, or nothing. */
export function focusKind(target: EventTarget | null): 'text' | 'control' | 'button' | null {
  const el = target as HTMLElement | null;
  if (!el || typeof el.closest !== 'function') return null;
  if (el.isContentEditable || el.closest('textarea')) return 'text';
  if (el.closest('select')) return 'control';
  const input = el.closest('input');
  if (input) return ['range', 'checkbox', 'radio', 'color', 'button', 'submit', 'reset', 'file'].includes(input.type) ? 'control' : 'text';
  return el.closest('button, a[href], summary') ? 'button' : null;
}

/** A combination as shown: ⌘ on a Mac, arrows and names spelled short. */
export function keyLabel(combo: string): string {
  const names: Record<string, string> = { ArrowLeft: '←', ArrowRight: '→', ArrowUp: '↑', ArrowDown: '↓', Escape: 'Esc', Delete: 'Del', PageUp: 'PgUp', PageDown: 'PgDn', Backspace: '⌫', Enter: 'Enter', Space: 'Space' };
  return combo
    .split('+')
    .map((p) => (p === 'Ctrl' ? (mac ? '⌘' : 'Ctrl') : (names[p] ?? p)))
    .join(mac ? '' : '+');
}

/** The window's key handler over a list of bindings (read afresh on every press). While
 * `modal` holds, only Escape gets through. */
export function createKeyHandler(list: () => Shortcut[], modal: () => boolean) {
  return (e: KeyboardEvent) => {
    if (e.defaultPrevented || e.isComposing || e.altKey) return;
    const combo = comboOf(e);
    if (modal() && combo !== 'Escape') return;
    const focus = focusKind(e.target);
    if (focus === 'control' && CONTROL_KEYS.has(e.key)) return;
    if (focus === 'button' && (e.key === 'Enter' || e.key === ' ')) return;
    const live = list().filter((s) => s.keys.includes(combo) && (focus !== 'text' || s.inInputs) && (!s.when || s.when()));
    if (import.meta.env.DEV && live.filter((s) => !s.hidden).length > 1) console.warn('[keys] more than one binding for', combo, live.map((s) => s.label));
    for (const s of live) {
      if (e.repeat && !s.repeat) {
        e.preventDefault();
        return;
      }
      if (s.run(e) === false) continue;
      e.preventDefault();
      return;
    }
  };
}
