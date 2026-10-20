(async () => {
  const counter = document.querySelector('[data-download-statistics]');
  if (!counter) return;
  try {
    const response = await fetch(counter.dataset.downloadStatistics, {
      cache: 'no-cache',
      signal: AbortSignal.timeout(5000),
    });
    if (!response.ok) return;
    const { total } = await response.json();
    if (!Number.isSafeInteger(total) || total < 0) return;
    counter.querySelector('[data-download-total]').textContent = new Intl.NumberFormat(
      document.documentElement.lang,
    ).format(total);
    counter.hidden = false;
  } catch {
    // При недоступности статистики сохраняем число, полученное при сборке страницы.
  }
})();
