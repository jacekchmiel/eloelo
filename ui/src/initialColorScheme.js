const prefersDarkMode = window.matchMedia(
  "(prefers-color-scheme: dark)",
).matches;

if (prefersDarkMode) {
  // apply dark background to reduce unstyled document flash
  // before react initial render
  document.body.style.backgroundColor = "#212121";

  // reset background color after safe margin for first meaningful paint
  setTimeout(() => {
    document.body.style.backgroundColor = "";
  }, 500);
}
