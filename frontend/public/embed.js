/*
 * Vantedge website widgets. Put this on any page:
 *
 *   <div data-vantedge="listings" data-tenant="your-workspace"></div>
 *   <script async src="https://YOUR-VANTEDGE-HOST/embed.js"></script>
 *
 * Widgets: listings, tour, reviews, map. Options (data- attributes):
 *   data-tenant   your workspace's short name (required)
 *   data-listing  a listing id, for the tour form
 *   data-map      a site map id, for the map
 *   data-accent   a hex colour such as #0e7c86, to match your site
 *   data-mode     light (default) or dark
 *
 * Each widget is an iframe served from Vantedge, so nothing here can touch
 * your page and your page cannot touch it. The iframe tells this script how
 * tall it is, and the script resizes it.
 */
(function () {
  "use strict";
  var script = document.currentScript;
  if (!script || !script.src) return;
  var base = new URL(script.src).origin;
  var WIDGETS = { listings: 1, tour: 1, reviews: 1, map: 1 };
  var frames = {};
  var seq = 0;

  function param(el, name) {
    var v = el.getAttribute("data-" + name);
    return v ? v.trim() : "";
  }

  function mount(el) {
    if (el.getAttribute("data-vantedge-mounted")) return;
    var widget = param(el, "vantedge");
    var tenant = param(el, "tenant");
    if (!WIDGETS[widget] || !tenant) {
      el.textContent = "Vantedge widget: set data-vantedge and data-tenant.";
      return;
    }
    el.setAttribute("data-vantedge-mounted", "1");
    var id = "vx" + ++seq;
    var q = new URLSearchParams({ tenant: tenant, id: id });
    ["listing", "map", "accent", "mode"].forEach(function (k) {
      var v = param(el, k);
      if (v) q.set(k, v);
    });
    var f = document.createElement("iframe");
    f.src = base + "/embed/" + widget + "?" + q.toString();
    f.title = "Vantedge " + widget;
    f.loading = "lazy";
    f.setAttribute("scrolling", "no");
    f.style.cssText = "width:100%;border:0;display:block;min-height:120px;";
    // Plain `sandbox` keeps the frame from navigating the host page.
    f.setAttribute(
      "sandbox",
      "allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"
    );
    frames[id] = f;
    el.appendChild(f);
  }

  window.addEventListener("message", function (e) {
    if (e.origin !== base) return;
    var d = e.data;
    if (!d || d.type !== "vantedge:height") return;
    var f = frames[d.id];
    if (!f || e.source !== f.contentWindow) return;
    var h = Number(d.height);
    if (h > 0 && h < 20000) f.style.height = Math.ceil(h) + "px";
  });

  function run() {
    var els = document.querySelectorAll("[data-vantedge]");
    for (var i = 0; i < els.length; i++) mount(els[i]);
    // A widget named on the script tag itself mounts right after it.
    if (script.getAttribute("data-vantedge") && !script.getAttribute("data-vantedge-mounted")) {
      var holder = document.createElement("div");
      ["vantedge", "tenant", "listing", "map", "accent", "mode"].forEach(function (k) {
        var v = script.getAttribute("data-" + k);
        if (v) holder.setAttribute("data-" + k, v);
      });
      script.setAttribute("data-vantedge-mounted", "1");
      script.parentNode.insertBefore(holder, script.nextSibling);
      mount(holder);
    }
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", run);
  else run();
})();
