(function () {
  var STANDALONE = ["/roadmap", "/changelog"];
  var TAB_KEY = "stnav-prev-tab";
  var NAV_KEY = "stnav-prev-nav";

  function getPath() {
    return document.documentElement.getAttribute("data-current-path") || "/";
  }

  function isStandalone(path) {
    return STANDALONE.indexOf(path) !== -1;
  }

  function getNavContainer() {
    return document.querySelector("#navigation-items");
  }

  document.addEventListener("click", function (e) {
    var href = null;
    for (var i = 0; i < STANDALONE.length; i++) {
      if (e.target.closest('a[href="' + STANDALONE[i] + '"]')) {
        var a = e.target.closest('a[href="' + STANDALONE[i] + '"]');
        if (!a.classList.contains("nav-tabs-item")) { href = STANDALONE[i]; break; }
      }
    }
    if (!href) return;

    var active = document.querySelector('a.nav-tabs-item[data-rm-active="true"]')
              || document.querySelector('a.nav-tabs-item[data-active="true"]');
    if (active) sessionStorage.setItem(TAB_KEY, active.getAttribute("href"));

    var nav = getNavContainer();
    if (nav) {
      var groups = nav.querySelectorAll(":scope > div");
      var html = "";
      groups.forEach(function (g) { html += g.outerHTML; });
      sessionStorage.setItem(NAV_KEY, html);
    }
  });

  function apply() {
    document.querySelectorAll("[data-rm-active]").forEach(function (el) {
      el.removeAttribute("data-rm-active");
    });
    document.documentElement.removeAttribute("data-rm-tab");

    if (!isStandalone(getPath())) return;

    var href = sessionStorage.getItem(TAB_KEY);
    if (!href) return;

    document.documentElement.setAttribute("data-rm-tab", "");
    var tab = document.querySelector('a.nav-tabs-item[href="' + href + '"]');
    if (tab) tab.setAttribute("data-rm-active", "true");

    var savedNav = sessionStorage.getItem(NAV_KEY);
    var nav = getNavContainer();
    if (savedNav && nav) {
      var existing = nav.querySelectorAll(":scope > div");
      existing.forEach(function (el) { el.remove(); });
      nav.insertAdjacentHTML("beforeend", savedNav);
    }
  }

  new MutationObserver(apply).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-current-path"],
  });

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", apply);
  } else {
    apply();
  }
})();
