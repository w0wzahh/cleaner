/* Cleaner — website scripts
   MIT License — Copyright (c) 2026 0xEmre */

(function () {
  "use strict";

  var reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var finePointer = window.matchMedia("(pointer: fine)").matches;

  /* ---------- Mobile nav ---------- */

  var toggle = document.querySelector(".nav-toggle");
  var links = document.getElementById("nav-links");

  function closeMenu() {
    if (!links) return;
    links.classList.remove("open");
    if (toggle) {
      toggle.classList.remove("open");
      toggle.setAttribute("aria-expanded", "false");
    }
  }

  if (toggle && links) {
    toggle.addEventListener("click", function () {
      var open = links.classList.toggle("open");
      toggle.classList.toggle("open", open);
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
    });
    links.addEventListener("click", function (e) {
      if (e.target.closest("a")) closeMenu();
    });
    document.addEventListener("click", function (e) {
      if (!e.target.closest(".nav-inner")) closeMenu();
    });
    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape") closeMenu();
    });
  }

  /* ---------- Active nav link ---------- */

  var page = location.pathname.split("/").pop() || "index.html";
  document.querySelectorAll(".nav-links a").forEach(function (a) {
    var href = a.getAttribute("href");
    if (href === page) a.classList.add("active");
  });

  /* ---------- Scroll reveal ---------- */

  var revealEls = document.querySelectorAll(".reveal");
  if ("IntersectionObserver" in window && !reduceMotion) {
    var io = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (entry.isIntersecting) {
            entry.target.classList.add("visible");
            io.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.12, rootMargin: "0px 0px -40px 0px" }
    );
    revealEls.forEach(function (el) {
      io.observe(el);
    });
  } else {
    revealEls.forEach(function (el) {
      el.classList.add("visible");
    });
  }

  /* ---------- Card spotlight ---------- */

  if (!reduceMotion && finePointer) {
    document.querySelectorAll(".card, .dl-card").forEach(function (card) {
      card.addEventListener("mousemove", function (e) {
        var r = card.getBoundingClientRect();
        card.style.setProperty("--mx", ((e.clientX - r.left) / r.width) * 100 + "%");
        card.style.setProperty("--my", ((e.clientY - r.top) / r.height) * 100 + "%");
      });
    });
  }

  /* ---------- Hero 3D tilt ---------- */

  var hero = document.querySelector(".hero");
  var visual = document.querySelector(".hero-visual");
  if (hero && visual && !reduceMotion && finePointer) {
    var tX = 0, tY = 0, cX = 0, cY = 0, rafId = null;
    var loop = function () {
      cX += (tX - cX) * 0.08;
      cY += (tY - cY) * 0.08;
      visual.style.transform =
        "rotateY(" + cX.toFixed(2) + "deg) rotateX(" + cY.toFixed(2) + "deg)";
      if (Math.abs(tX - cX) > 0.02 || Math.abs(tY - cY) > 0.02) {
        rafId = requestAnimationFrame(loop);
      } else {
        rafId = null;
      }
    };
    hero.addEventListener("mousemove", function (e) {
      var r = visual.getBoundingClientRect();
      tX = ((e.clientX - (r.left + r.width / 2)) / r.width) * 10;
      tY = -((e.clientY - (r.top + r.height / 2)) / r.height) * 8;
      if (!rafId) rafId = requestAnimationFrame(loop);
    });
    hero.addEventListener("mouseleave", function () {
      tX = 0;
      tY = 0;
      if (!rafId) rafId = requestAnimationFrame(loop);
    });
  }

  /* ---------- Latest release download links ---------- */

  /* Buttons start with pinned URLs so they work without JS or if the API
     is unreachable; this rewrites them to the newest release when possible. */
  var dlLinks = document.querySelectorAll("[data-dl]");
  if (dlLinks.length && window.fetch) {
    fetch("https://api.github.com/repos/w0wzahh/cleaner/releases/latest")
      .then(function (r) {
        return r.ok ? r.json() : Promise.reject(new Error("http " + r.status));
      })
      .then(function (data) {
        var exe = null,
          zip = null;
        (data.assets || []).forEach(function (a) {
          if (/\.exe$/i.test(a.name)) exe = a.browser_download_url;
          else if (/\.zip$/i.test(a.name)) zip = a.browser_download_url;
        });
        dlLinks.forEach(function (a) {
          var kind = a.getAttribute("data-dl");
          if (kind === "installer" && exe) a.href = exe;
          if (kind === "portable" && zip) a.href = zip;
        });
        var badge = document.querySelector("[data-rel-version]");
        if (badge && data.tag_name) badge.textContent = data.tag_name;
      })
      .catch(function () {
        /* keep the pinned fallback URLs */
      });
  }

  /* ---------- Nav shrink on scroll ---------- */

  var navInner = document.querySelector(".nav-inner");
  if (navInner) {
    var onScroll = function () {
      navInner.style.boxShadow =
        window.scrollY > 30
          ? "0 12px 36px rgba(0, 0, 0, 0.5)"
          : "0 8px 32px rgba(0, 0, 0, 0.3)";
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
  }
})();
