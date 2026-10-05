// Can this browser show the map? Checked before the app mounts, so a visitor without WebGL2
// reads why instead of looking at a blank page.

/** True if the map can run; else a plain message is put in `target`. */
export function checkSupport(target: HTMLElement): boolean {
  let ok = false;
  try {
    ok = !!document.createElement('canvas').getContext('webgl2');
  } catch {
    // Treated as no WebGL2.
  }
  if (ok) return true;
  target.innerHTML = `<div style="max-width:32rem;margin:15vh auto;padding:0 16px;font:16px/1.5 system-ui,sans-serif">
  <h1 style="font-size:1.4rem">Worldspring can't run here</h1>
  <p>It draws the map with WebGL2, which this browser has turned off or doesn't have. Try a current Chrome, Edge,
  Firefox or Safari, and check that hardware acceleration is on.</p></div>`;
  return false;
}

/** The device says it has little memory (big cities and battlemaps may then crash the tab). */
export function lowMemory(): boolean {
  const gb = (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
  return gb !== undefined && gb <= 2;
}
