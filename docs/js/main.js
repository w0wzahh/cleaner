/* Cleaner — website scripts (neobrutalist)
   MIT License — Copyright (c) 2026 0xEmre */

(() => {
  "use strict";

  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const fine = matchMedia("(pointer: fine)").matches;

  /* ---------- custom neobrutal cursor — identical to w0wzahh.link:
     white arrow + black offset shadow, turns yellow + tilts -14deg over
     interactive elements, squashes on click. The outer div only tracks the
     pointer; an inner wrapper carries the scale/rotate so the transform
     origin stays pinned at the arrow tip and the hotspot never drifts. -- */

  if (fine && !reduced) {
    const cur = document.createElement("div");
    cur.id = "cursor";
    cur.innerHTML =
      '<div class="cursor-arrow">' +
      '<svg width="30" height="34" viewBox="0 0 30 34">' +
      '<path d="M7 5 L23 18 L16.8 19.2 L20.2 27.8 L16.9 29.2 L13.4 20.8 L7 25 Z" transform="translate(2.5 2.5)" fill="#000"/>' +
      '<path class="arrow" d="M7 5 L23 18 L16.8 19.2 L20.2 27.8 L16.9 29.2 L13.4 20.8 L7 25 Z" stroke="#000" stroke-width="2.5" stroke-linejoin="round"/>' +
      "</svg></div>";
    document.body.appendChild(cur);
    document.body.classList.add("has-cursor");

    let tx = -100, ty = -100, cx = -100, cy = -100;
    addEventListener("pointermove", (e) => { tx = e.clientX; ty = e.clientY; });
    addEventListener("pointerover", (e) => {
      cur.classList.toggle(
        "hover",
        !!e.target.closest?.("a, button, [role='button'], input, textarea, select, label, [data-cursor]")
      );
    });
    addEventListener("pointerdown", () => cur.classList.add("down"));
    addEventListener("pointerup", () => cur.classList.remove("down"));

    (function follow() {
      cx += (tx - cx) * 0.3;
      cy += (ty - cy) * 0.3;
      cur.style.transform = `translate(${cx}px, ${cy}px)`;
      requestAnimationFrame(follow);
    })();
  }

  /* ---------- scroll progress bar ---------- */

  const bar = document.getElementById("progressbar");
  if (bar) {
    let target = 0, cur2 = 0;
    addEventListener("scroll", () => {
      const max = document.documentElement.scrollHeight - innerHeight;
      target = max > 0 ? scrollY / max : 0;
    }, { passive: true });
    (function slide() {
      cur2 += (target - cur2) * 0.12;
      bar.style.transform = `scaleX(${cur2})`;
      requestAnimationFrame(slide);
    })();
  }

  /* ---------- mobile nav ---------- */

  const toggle = document.querySelector(".nav-toggle");
  const links = document.getElementById("nav-links");

  function closeMenu() {
    if (!links) return;
    links.classList.remove("open");
    if (toggle) toggle.setAttribute("aria-expanded", "false");
  }

  if (toggle && links) {
    toggle.addEventListener("click", () => {
      const open = links.classList.toggle("open");
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
    });
    links.addEventListener("click", (e) => {
      if (e.target.closest("a")) closeMenu();
    });
    document.addEventListener("click", (e) => {
      if (!e.target.closest(".navbar-inner")) closeMenu();
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") closeMenu();
    });
  }

  /* ---------- active nav link ---------- */

  const page = location.pathname.split("/").pop() || "index.html";
  document.querySelectorAll(".nav-links a").forEach((a) => {
    if (a.getAttribute("href") === page) a.classList.add("active");
  });

  /* ---------- scroll-triggered reveals — .rv → .rv-in with sibling stagger ---------- */

  if (!reduced && "IntersectionObserver" in window) {
    const io = new IntersectionObserver((entries) => {
      entries.forEach((e) => {
        if (e.isIntersecting) {
          e.target.classList.add("rv-in");
          io.unobserve(e.target);
        }
      });
    }, { threshold: 0.15 });

    const seen = new Map();
    document
      .querySelectorAll(".stat, .card, .dl-card, .legal-sec, .docs-content section, .sec-head h2")
      .forEach((el) => {
        const n = seen.get(el.parentElement) || 0;
        seen.set(el.parentElement, n + 1);
        el.style.transitionDelay = `${Math.min(n * 70, 560)}ms`;
        el.classList.add("rv");
        io.observe(el);
      });
  }

  /* ---------- latest release download links ---------- */

  /* Buttons start with pinned URLs so they work without JS or if the API
     is unreachable; this rewrites them to the newest release when possible. */
  const dlLinks = document.querySelectorAll("[data-dl]");
  if (dlLinks.length && window.fetch) {
    fetch("https://api.github.com/repos/w0wzahh/cleaner/releases/latest")
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error("http " + r.status))))
      .then((data) => {
        let exe = null, zip = null;
        (data.assets || []).forEach((a) => {
          if (/\.exe$/i.test(a.name)) exe = a.browser_download_url;
          else if (/\.zip$/i.test(a.name)) zip = a.browser_download_url;
        });
        dlLinks.forEach((a) => {
          const kind = a.getAttribute("data-dl");
          if (kind === "installer" && exe) a.href = exe;
          if (kind === "portable" && zip) a.href = zip;
        });
        document.querySelectorAll("[data-rel-version]").forEach((el) => {
          if (data.tag_name) el.textContent = data.tag_name;
        });
      })
      .catch(() => { /* keep the pinned fallback URLs */ });
  }

  /* ---------- TOC scrollspy ---------- */

  const tocLinks = document.querySelectorAll(".docs-toc a, .legal-nav a[href^='#']");
  if (tocLinks.length && "IntersectionObserver" in window) {
    const tocMap = {};
    tocLinks.forEach((a) => {
      const href = a.getAttribute("href") || "";
      if (href.charAt(0) === "#") tocMap[href.slice(1)] = a;
    });
    const spy = new IntersectionObserver((entries) => {
      entries.forEach((en) => {
        if (en.isIntersecting && tocMap[en.target.id]) {
          tocLinks.forEach((a) => a.classList.remove("toc-active"));
          tocMap[en.target.id].classList.add("toc-active");
        }
      });
    }, { rootMargin: "-18% 0px -72% 0px" });
    document
      .querySelectorAll(".legal-sec[id], .docs-content section[id]")
      .forEach((s) => { if (tocMap[s.id]) spy.observe(s); });
  }
})();
