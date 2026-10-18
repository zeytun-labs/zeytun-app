import { polyfillCountryFlagEmojis } from "country-flag-emoji-polyfill";
import flagFontUrl from "country-flag-emoji-polyfill/dist/TwemojiCountryFlags.woff2?url";

export function initFlagPolyfill() {
  if (typeof window === "undefined") return;
  polyfillCountryFlagEmojis("Twemoji Country Flags", flagFontUrl);
}
