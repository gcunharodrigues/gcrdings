'use client';

import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { log } from '@/lib/logger';

/**
 * Tailwind was configured with darkMode: ['class'] and the theme tokens for
 * both themes already existed in globals.css — but nothing ever put the class
 * on the document, so the dark palette was unreachable.
 */
export type ThemePreference = 'system' | 'light' | 'dark';

const STORAGE_KEY = 'themePreference';

interface ThemeContextValue {
  preference: ThemePreference;
  /** What is actually on screen once 'system' is resolved. */
  resolved: 'light' | 'dark';
  setPreference: (preference: ThemePreference) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
  const context = useContext(ThemeContext);
  if (!context) throw new Error('useTheme must be used within a ThemeProvider');
  return context;
}

function readStoredPreference(): ThemePreference {
  if (typeof window === 'undefined') return 'system';
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return stored === 'light' || stored === 'dark' || stored === 'system' ? stored : 'system';
  } catch (error) {
    log.warn('[theme] Failed to read the stored preference:', error);
    return 'system';
  }
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [preference, setPreferenceState] = useState<ThemePreference>('system');
  const [resolved, setResolved] = useState<'light' | 'dark'>('light');

  // Read after mount: localStorage and matchMedia do not exist during SSR, and
  // reading them in the initial state would mismatch the server render.
  useEffect(() => setPreferenceState(readStoredPreference()), []);

  useEffect(() => {
    const query = window.matchMedia('(prefers-color-scheme: dark)');

    const apply = () => {
      const next = preference === 'system' ? (query.matches ? 'dark' : 'light') : preference;
      setResolved(next);
      document.documentElement.classList.toggle('dark', next === 'dark');
      document.documentElement.style.colorScheme = next;
    };

    apply();
    // Only 'system' has to track the OS; a pinned choice must survive it changing.
    if (preference !== 'system') return;
    query.addEventListener('change', apply);
    return () => query.removeEventListener('change', apply);
  }, [preference]);

  const setPreference = useCallback((next: ThemePreference) => {
    setPreferenceState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch (error) {
      log.warn('[theme] Failed to store the preference:', error);
    }
  }, []);

  return (
    <ThemeContext.Provider value={{ preference, resolved, setPreference }}>
      {children}
    </ThemeContext.Provider>
  );
}
