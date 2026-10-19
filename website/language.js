(() => {
  const root = document.documentElement.dataset;
  const supported = (root.supportedLocales || 'ru,en').split(',');
  const fallback = root.defaultLocale || 'en';
  const storageKey = 'converter.site.language';
  document.querySelectorAll('[data-site-language]').forEach((link) => {
    link.addEventListener('click', () => {
      try {
        localStorage.setItem(storageKey, link.dataset.siteLanguage);
      } catch {
        /* Private browsing may disable storage. */
      }
    });
  });
  if (root.languageRoot !== 'true') return;
  let saved;
  try {
    saved = localStorage.getItem(storageKey);
  } catch {
    /* Use the primary browser language. */
  }
  const primary = (navigator.language || '').split(/[-_]/)[0].toLowerCase();
  const language = supported.includes(saved)
    ? saved
    : supported.includes(primary)
      ? primary
      : fallback;
  const url = new URL(language + '/', document.baseURI);
  url.search = location.search;
  url.hash = location.hash;
  location.replace(url.href);
})();
