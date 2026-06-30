(() => {
  const bootstrap = {
    interacted: false,
    pendingMode: null,
    pendingVerify: false,
  };

  window.__mfenxBootstrap = bootstrap;

  document.addEventListener(
    "click",
    (event) => {
      const target = event.target;
      if (!(target instanceof Element)) return;

      const modeButton = target.closest(".proof-mode[data-mode]");
      if (modeButton) {
        bootstrap.interacted = true;
        bootstrap.pendingMode = modeButton.dataset.mode || null;
      }

      if (target.closest("#verify-button")) {
        bootstrap.interacted = true;
        bootstrap.pendingVerify = true;
      }
    },
    { capture: true },
  );
})();
