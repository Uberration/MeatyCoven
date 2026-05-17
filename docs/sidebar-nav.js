(() => {
  const sidebar = document.getElementById('docs-sidebar');
  const overlay = document.querySelector('[data-sidebar-overlay]');
  const triggers = Array.from(document.querySelectorAll('[data-sidebar-trigger]'));
  const closeButtons = Array.from(document.querySelectorAll('[data-sidebar-close]'));
  const desktopQuery = window.matchMedia('(min-width: 860px)');
  const storageKey = 'opencoven.docs.sidebarCollapsed';

  if (!sidebar || !overlay || triggers.length === 0) return;

  function isDesktop() {
    return desktopQuery.matches;
  }

  function setTriggerState(expanded) {
    for (const trigger of triggers) {
      trigger.setAttribute('aria-expanded', String(expanded));
    }
  }

  function setMobileOpen(open) {
    sidebar.dataset.open = String(open);
    overlay.hidden = !open;
    document.body.classList.toggle('sidebar-open', open);
    setTriggerState(open);
  }

  function setDesktopCollapsed(collapsed) {
    document.body.classList.toggle('sidebar-collapsed', collapsed);
    setTriggerState(!collapsed);
    try {
      window.localStorage.setItem(storageKey, collapsed ? '1' : '0');
    } catch {
      // localStorage is optional; the button remains fully functional without it.
    }
  }

  function getStoredDesktopCollapsed() {
    try {
      return window.localStorage.getItem(storageKey) === '1';
    } catch {
      return false;
    }
  }

  function syncMode() {
    if (isDesktop()) {
      setMobileOpen(false);
      setDesktopCollapsed(getStoredDesktopCollapsed());
      return;
    }

    document.body.classList.remove('sidebar-collapsed');
    setMobileOpen(false);
  }

  function toggleSidebar() {
    if (isDesktop()) {
      setDesktopCollapsed(!document.body.classList.contains('sidebar-collapsed'));
      return;
    }

    setMobileOpen(sidebar.dataset.open !== 'true');
  }

  for (const trigger of triggers) {
    trigger.addEventListener('click', toggleSidebar);
  }

  for (const closeButton of closeButtons) {
    closeButton.addEventListener('click', () => setMobileOpen(false));
  }

  overlay.addEventListener('click', () => setMobileOpen(false));

  sidebar.addEventListener('click', (event) => {
    if (!isDesktop() && event.target instanceof HTMLAnchorElement) {
      setMobileOpen(false);
    }
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && !isDesktop() && sidebar.dataset.open === 'true') {
      setMobileOpen(false);
      triggers[0]?.focus();
    }
  });

  desktopQuery.addEventListener('change', syncMode);
  syncMode();
})();
