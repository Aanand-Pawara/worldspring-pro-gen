import type { Section, Tabs, Snap } from './layout.svelte';

export type UiNavigation = {
  section: Section | null;
  tabs: Tabs;
  collapsed: boolean;
  snap: Snap;
  card: boolean;
  menu: 'main' | 'layers' | null;
};

export function openNavigation(state: UiNavigation, section: Section, tab?: string): void {
  state.section = section;
  if (tab !== undefined && section !== 'play') (state.tabs as Record<string, string>)[section] = tab;
  state.collapsed = false;
  state.snap = 'half';
  state.card = false;
  state.menu = null;
}

export function closeNavigation(state: UiNavigation): void {
  state.section = null;
  state.collapsed = false;
  state.snap = 'half';
  state.card = false;
  state.menu = null;
}

export function toggleNavigation(state: UiNavigation, section: Section): boolean {
  if (state.section === section) {
    if (state.collapsed) {
      state.collapsed = false;
      return true;
    }
    closeNavigation(state);
    return false;
  }
  openNavigation(state, section);
  return true;
}

export function openTab(state: UiNavigation, section: Exclude<Section, 'play'>, tab: string): void {
  if (state.section === section && state.tabs[section] === tab) {
    closeNavigation(state);
    return;
  }
  openNavigation(state, section, tab);
}
