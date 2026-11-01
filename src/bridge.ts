import { invoke } from '@tauri-apps/api/core';
import { createDemoBackend } from './demo';
import type { Backend, College, Settings } from './types';

export const isDesktop = '__TAURI_INTERNALS__' in window;

function createDesktopBackend(): Backend {
  return {
    desktop: true,
    getSnapshot: () => invoke('get_snapshot'),
    fetchCatalog: (college: College) => invoke('fetch_catalog', { college }),
    saveSettings: (settings: Settings) => invoke('save_settings', { settings }),
    getCachedDay: (date: string) => invoke('get_cached_day', { date }),
    loadDay: (date: string) => invoke('load_day', { date }),
    refreshNow: () => invoke('refresh_now'),
    revealWindow: () => invoke('reveal_window'),
    openOnSite: (date: string) => invoke('open_on_site', { date }),
  };
}

export const backend = isDesktop ? createDesktopBackend() : createDemoBackend();
