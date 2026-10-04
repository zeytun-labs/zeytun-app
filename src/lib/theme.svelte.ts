export type Theme = "dark" | "light" | "system";

export const THEME_STORAGE_KEY = "zeytun-theme";

export const themeOptions: { label: string; value: Theme }[] = [
  { label: "Dark", value: "dark" },
  { label: "Light", value: "light" },
  { label: "System", value: "system" },
];

function isTheme(value: string | null): value is Theme {
  return value === "dark" || value === "light" || value === "system";
}

class ThemePreference {
  value = $state<Theme>("dark");
  ready = $state(false);
  #mediaQuery: MediaQueryList | null = null;

  init() {
    if (typeof window === "undefined" || this.ready) {
      return;
    }

    // Force dark mode for now
    this.value = "dark";
    localStorage.setItem(THEME_STORAGE_KEY, "dark");
    this.apply();
    this.ready = true;

    /*
    const storedTheme = localStorage.getItem(THEME_STORAGE_KEY);

    if (isTheme(storedTheme)) {
      this.value = storedTheme;
    }

    this.apply();
    this.ready = true;

    this.#mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    this.#mediaQuery.addEventListener("change", this.#handleSystemThemeChange);
    */
  }

  set(value: Theme) {
    this.value = value;
    localStorage.setItem(THEME_STORAGE_KEY, value);
    this.apply();
  }

  apply() {
    // Force dark mode for now
    document.documentElement.classList.add("dark");

    /*
    const shouldUseDark =
      this.value === "dark" ||
      (this.value === "system" &&
        window.matchMedia("(prefers-color-scheme: dark)").matches);

    document.documentElement.classList.toggle("dark", shouldUseDark);
    */
  }

  #handleSystemThemeChange = () => {
    if (this.value === "system") {
      this.apply();
    }
  };
}

export const themePreference = new ThemePreference();
