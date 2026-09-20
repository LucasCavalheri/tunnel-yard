/** sessionStorage key for the scroll offset saved across a language switch. */
export const LOCALE_SCROLL_KEY = "tunnelyard-locale-scroll";

/** Ignore a leftover payload if the navigation took longer than this. */
export const LOCALE_SCROLL_TTL_MS = 8000;

/**
 * @param {number} scrollY
 * @param {number} [now]
 * @returns {string}
 */
export function encodeLocaleScroll(scrollY, now = Date.now()) {
  return JSON.stringify({
    y: Math.max(0, Math.round(Number(scrollY) || 0)),
    t: now,
  });
}

/**
 * @param {string | null} raw
 * @param {number} [now]
 * @returns {number | null}
 */
export function decodeLocaleScroll(raw, now = Date.now()) {
  if (!raw) return null;
  try {
    const data = JSON.parse(raw);
    if (typeof data.y !== "number" || !Number.isFinite(data.y) || data.y < 0) return null;
    if (typeof data.t !== "number" || now - data.t > LOCALE_SCROLL_TTL_MS || now < data.t) {
      return null;
    }
    return Math.round(data.y);
  } catch {
    return null;
  }
}
