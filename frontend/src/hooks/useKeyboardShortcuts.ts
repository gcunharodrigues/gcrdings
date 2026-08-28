'use client';

import { useEffect } from 'react';

export interface KeyboardShortcut {
  /** Lowercase key, e.g. 'k'. Matched against KeyboardEvent.key. */
  key: string;
  /** Requires Cmd on macOS / Ctrl elsewhere. */
  mod?: boolean;
  shift?: boolean;
  handler: () => void;
  /** Fire even while a text field has focus. Off by default. */
  allowInInput?: boolean;
}

function isTextEntryTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}

/**
 * Binds global shortcuts. Modifier shortcuts use Cmd on macOS and Ctrl
 * elsewhere, matching what each platform's users already expect.
 */
export function useKeyboardShortcuts(shortcuts: KeyboardShortcut[], enabled = true) {
  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      const usesMac = navigator.platform.toUpperCase().includes('MAC');
      const modPressed = usesMac ? event.metaKey : event.ctrlKey;

      for (const shortcut of shortcuts) {
        if (event.key.toLowerCase() !== shortcut.key.toLowerCase()) continue;
        if (Boolean(shortcut.mod) !== modPressed) continue;
        if (Boolean(shortcut.shift) !== event.shiftKey) continue;
        if (!shortcut.allowInInput && isTextEntryTarget(event.target)) continue;

        event.preventDefault();
        shortcut.handler();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [shortcuts, enabled]);
}
