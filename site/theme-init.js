// the theme picked with the button in the app bar (light / dark), before the page is drawn;
// nothing stored = follow the system
try { const t = localStorage.getItem("theme"); if (t === "light" || t === "dark") document.documentElement.dataset.theme = t; } catch {}
