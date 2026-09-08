"use strict";
/* Apply before CSS and before the first render. Dark is intentional, not OS-dependent. */
(function () {
  const key = 'tundra-appearance-v1';
  let value = 'dark';
  try {
    const current = window.localStorage.getItem(key);
    const saved = current === null ? window.localStorage.getItem('relay-appearance-v1') : current;
    if (saved === 'light' || saved === 'dark') {
      value = saved;
      if (current === null) { try { window.localStorage.setItem(key, value); } catch (_) {} }
    }
  } catch (_) {}
  function apply(next, save) {
    if (next !== 'light' && next !== 'dark') return false;
    value = next;
    document.documentElement.dataset.theme = value;
    document.documentElement.style.colorScheme = value;
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.content = value === 'dark' ? '#0e1115' : '#fbfcfd';
    let persisted = false;
    if (save) { try { window.localStorage.setItem(key, value); persisted = true; } catch (_) {} }
    window.dispatchEvent(new CustomEvent('tundra:themechange', {detail: {theme: value, persisted}}));
    return true;
  }
  window.TundraTheme = Object.freeze({get: () => value, set: next => apply(next, true), toggle: () => apply(value === 'dark' ? 'light' : 'dark', true), storageKey: key});
  window.addEventListener('storage', event => { if (event.key === key || event.key === null) apply(event.newValue === 'light' ? 'light' : 'dark', false); });
  apply(value, false);
})();
