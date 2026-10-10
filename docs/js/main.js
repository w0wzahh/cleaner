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

  /* ---------- Auto-stagger reveals ---------- */

  /* Grid children without an explicit reveal-dN class get a cascade delay
     so cards pop in one after another instead of all at once. */
  document
    .querySelectorAll(".card-grid, .legal-docs, .perm-grid, .dl-grid")
    .forEach(function (group) {
      var i = 0;
      Array.prototype.forEach.call(group.children, function (el) {
        if (el.classList.contains("reveal") && !/reveal-d\d/.test(el.className)) {
          el.style.transitionDelay = Math.min(i * 70, 560) + "ms";
        }
        i++;
      });
    });

  /* ---------- Magnetic buttons ---------- */

  if (!reduceMotion && finePointer) {
    document.querySelectorAll(".btn").forEach(function (btn) {
      btn.addEventListener("mousemove", function (e) {
        var r = btn.getBoundingClientRect();
        var x = (e.clientX - r.left - r.width / 2) / r.width;
        var y = (e.clientY - r.top - r.height / 2) / r.height;
        btn.style.transform =
          "translate(" + (x * 6).toFixed(1) + "px," + (y * 5).toFixed(1) + "px)";
      });
      btn.addEventListener("mouseleave", function () {
        btn.style.transform = "";
      });
    });
  }

  /* ---------- Button click ripple ---------- */

  document.addEventListener("click", function (e) {
    if (reduceMotion) return;
    var btn = e.target.closest(".btn");
    if (!btn) return;
    var r = btn.getBoundingClientRect();
    var d = Math.max(r.width, r.height);
    var s = document.createElement("span");
    s.className = "ripple";
    s.style.width = s.style.height = d + "px";
    s.style.left = e.clientX - r.left - d / 2 + "px";
    s.style.top = e.clientY - r.top - d / 2 + "px";
    btn.appendChild(s);
    setTimeout(function () {
      s.remove();
    }, 650);
  });

  /* ---------- Scroll progress + back-to-top ---------- */

  var progress = document.createElement("div");
  progress.className = "scroll-progress";
  document.body.appendChild(progress);

  var toTop = document.createElement("button");
  toTop.className = "to-top";
  toTop.setAttribute("aria-label", "Back to top");
  toTop.innerHTML =
    '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/></svg>';
  document.body.appendChild(toTop);
  toTop.addEventListener("click", function () {
    window.scrollTo({ top: 0, behavior: reduceMotion ? "auto" : "smooth" });
  });

  var doc = document.documentElement;
  window.addEventListener(
    "scroll",
    function () {
      var max = doc.scrollHeight - doc.clientHeight;
      progress.style.width = (max > 0 ? (doc.scrollTop / max) * 100 : 0) + "%";
      toTop.classList.toggle("show", doc.scrollTop > 600);
    },
    { passive: true }
  );

  /* ---------- Blob cursor parallax ---------- */

  /* Feeds --bx/--by on <html>; CSS `translate` picks them up per-blob at
     different depths. Independent of the keyframed `transform` drift. */
  if (!reduceMotion && finePointer && document.querySelector(".blob")) {
    var bmx = 0,
      bmy = 0,
      bcx = 0,
      bcy = 0,
      braf = null;
    var blobLoop = function () {
      bcx += (bmx - bcx) * 0.045;
      bcy += (bmy - bcy) * 0.045;
      doc.style.setProperty("--bx", bcx.toFixed(1) + "px");
      doc.style.setProperty("--by", bcy.toFixed(1) + "px");
      if (Math.abs(bmx - bcx) > 0.05 || Math.abs(bmy - bcy) > 0.05) {
        braf = requestAnimationFrame(blobLoop);
      } else {
        braf = null;
      }
    };
    document.addEventListener(
      "mousemove",
      function (e) {
        bmx = (e.clientX / window.innerWidth - 0.5) * 46;
        bmy = (e.clientY / window.innerHeight - 0.5) * 46;
        if (!braf) braf = requestAnimationFrame(blobLoop);
      },
      { passive: true }
    );
  }

  /* ---------- Cursor glow ---------- */

  if (!reduceMotion && finePointer) {
    var glow = document.createElement("div");
    glow.className = "cursor-glow";
    document.body.appendChild(glow);
    var gx = window.innerWidth / 2,
      gy = window.innerHeight / 2,
      gcx = gx,
      gcy = gy,
      graf = null;
    var glowLoop = function () {
      gcx += (gx - gcx) * 0.12;
      gcy += (gy - gcy) * 0.12;
      glow.style.transform =
        "translate(" + gcx.toFixed(1) + "px," + gcy.toFixed(1) + "px)";
      if (Math.abs(gx - gcx) > 0.5 || Math.abs(gy - gcy) > 0.5) {
        graf = requestAnimationFrame(glowLoop);
      } else {
        graf = null;
      }
    };
    document.addEventListener(
      "mousemove",
      function (e) {
        gx = e.clientX;
        gy = e.clientY;
        glow.classList.add("on");
        if (!graf) graf = requestAnimationFrame(glowLoop);
      },
      { passive: true }
    );
    document.addEventListener("mouseleave", function () {
      glow.classList.remove("on");
    });
  }

  /* ---------- TOC scrollspy ---------- */

  var tocLinks = document.querySelectorAll(".docs-toc a");
  if (tocLinks.length && "IntersectionObserver" in window) {
    var tocMap = {};
    tocLinks.forEach(function (a) {
      var href = a.getAttribute("href") || "";
      if (href.charAt(0) === "#") tocMap[href.slice(1)] = a;
    });
    var spy = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (en) {
          if (en.isIntersecting && tocMap[en.target.id]) {
            tocLinks.forEach(function (a) {
              a.classList.remove("toc-active");
            });
            tocMap[en.target.id].classList.add("toc-active");
          }
        });
      },
      { rootMargin: "-18% 0px -72% 0px" }
    );
    document
      .querySelectorAll(".legal-sec[id], .docs-content section[id]")
      .forEach(function (s) {
        if (tocMap[s.id]) spy.observe(s);
      });
  }

})();
