(() => {
  const downloads = document.querySelector('.downloads');
  if (!downloads) return;
  const agent = navigator.userAgent || '';
  const platform = navigator.userAgentData?.platform || navigator.platform || '';
  const mobile =
    navigator.userAgentData?.mobile ||
    /Android|iPhone|iPad|iPod|Mobile/i.test(agent) ||
    (/Mac/i.test(platform) && navigator.maxTouchPoints > 1);
  if (mobile) return;
  const system =
    /Win/i.test(platform) || /Windows/i.test(agent)
      ? 'windows'
      : /Mac/i.test(platform) || /Macintosh/i.test(agent)
        ? 'macos'
        : null;
  if (!system) return;
  const primary = downloads.querySelector(`[data-download-system="${system}"]`);
  const options = downloads.querySelector('.download-options');
  const others = downloads.querySelector('.other-systems');
  if (!primary || !options || !others) return;
  primary.classList.remove('download-alternative');
  downloads.prepend(primary);
  others.append(options);
  others.hidden = false;
})();
