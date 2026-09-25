// 手册交互：关键词/主题切换、目录、复制、附录过滤。
// 所有内容在构建期生成，这里只做切换与筛选，不请求任何网络资源。

(function () {
  "use strict";

  var root = document.documentElement;
  var $ = function (selector, scope) { return (scope || document).querySelector(selector); };
  var $$ = function (selector, scope) {
    return Array.prototype.slice.call((scope || document).querySelectorAll(selector));
  };

  function store(key, value) {
    try { localStorage.setItem(key, value); } catch (error) { /* 隐私模式 */ }
  }
  function restore(key) {
    try { return localStorage.getItem(key); } catch (error) { return null; }
  }

  /* ---------- 关键词切换 ---------- */
  var keywordButton = $("#keyword-toggle");
  function applyKeywords(mode) {
    root.setAttribute("data-kw", mode);
    store("mclang-doc-keywords", mode);
    if (keywordButton) {
      keywordButton.textContent = mode === "zh" ? "English keywords" : "中文关键词";
      keywordButton.title =
        mode === "zh" ? "把示例切换为英文关键词" : "把示例切换为中文关键词";
    }
  }
  applyKeywords(root.getAttribute("data-kw") === "en" ? "en" : "zh");
  if (keywordButton) {
    keywordButton.addEventListener("click", function () {
      applyKeywords(root.getAttribute("data-kw") === "zh" ? "en" : "zh");
    });
  }

  /* ---------- 主题切换 ---------- */
  var themeButton = $("#theme-toggle");
  function applyTheme(theme) {
    root.setAttribute("data-theme", theme);
    store("mclang-doc-theme", theme);
    if (themeButton) themeButton.textContent = theme === "dark" ? "浅色" : "深色";
  }
  applyTheme(root.getAttribute("data-theme") === "dark" ? "dark" : "light");
  if (themeButton) {
    themeButton.addEventListener("click", function () {
      applyTheme(root.getAttribute("data-theme") === "dark" ? "light" : "dark");
    });
  }

  /* ---------- 移动端目录 ---------- */
  var toc = $("#toc");
  var navButton = $("#nav-toggle");
  function setNavOpen(open) {
    if (!toc || !navButton) return;
    var isMobile = window.matchMedia && window.matchMedia("(max-width: 980px)").matches;
    var isOpen = Boolean(open && isMobile);
    toc.classList.toggle("open", isOpen);
    toc.inert = Boolean(isMobile && !isOpen);
    navButton.setAttribute("aria-expanded", isOpen ? "true" : "false");
    navButton.setAttribute("aria-label", isOpen ? "关闭章节目录" : "打开章节目录");
  }
  if (navButton && toc) {
    setNavOpen(false);
    navButton.addEventListener("click", function () {
      setNavOpen(!toc.classList.contains("open"));
    });
    toc.addEventListener("click", function (event) {
      if (event.target.closest("a")) setNavOpen(false);
    });
    document.addEventListener("click", function (event) {
      if (
        toc.classList.contains("open") &&
        !toc.contains(event.target) &&
        !navButton.contains(event.target)
      ) setNavOpen(false);
    });
    window.addEventListener("resize", function () {
      setNavOpen(toc.classList.contains("open"));
    });
  }

  /* ---------- 目录高亮当前章节 ---------- */
  var tocLinks = toc ? $$(".toc-links a", toc) : [];
  var sections = tocLinks
    .map(function (link) {
      var id = link.getAttribute("href").slice(1);
      var heading = document.getElementById(id);
      return heading ? { link: link, heading: heading } : null;
    })
    .filter(Boolean);
  if (sections.length > 0 && "IntersectionObserver" in window) {
    var visible = new Set();
    var observer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (entry.isIntersecting) visible.add(entry.target.id);
          else visible.delete(entry.target.id);
        });
        var first = sections.find(function (item) { return visible.has(item.heading.id); });
        sections.forEach(function (item) {
          item.link.classList.toggle("active", Boolean(first) && item === first);
        });
      },
      { rootMargin: "-70px 0px -70% 0px", threshold: 0 },
    );
    sections.forEach(function (item) { observer.observe(item.heading); });
  }

  /* ---------- 目录搜索 ---------- */
  var tocSearch = $("#toc-search");
  var tocEmpty = $("#toc-empty");
  if (tocSearch) {
    tocSearch.addEventListener("input", function () {
      var query = tocSearch.value.trim().toLocaleLowerCase();
      var matches = 0;
      tocLinks.forEach(function (link) {
        var match = query === "" || link.textContent.toLocaleLowerCase().indexOf(query) !== -1;
        link.hidden = !match;
        if (match) matches += 1;
      });
      if (tocEmpty) tocEmpty.hidden = matches > 0;
    });
  }

  document.addEventListener("keydown", function (event) {
    var target = event.target;
    var isEditing = target && (
      target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName)
    );
    if (event.key === "Escape") {
      if (tocSearch && tocSearch.value) {
        tocSearch.value = "";
        tocSearch.dispatchEvent(new Event("input", { bubbles: true }));
      }
      setNavOpen(false);
      if (tocSearch && document.activeElement === tocSearch) tocSearch.blur();
    } else if (event.key === "/" && !isEditing && !event.altKey && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      if (tocSearch) {
        setNavOpen(true);
        tocSearch.focus();
      }
    }
  });

  /* ---------- 回到顶部 ---------- */
  var toTop = $("#to-top");
  if (toTop) {
    window.addEventListener("scroll", function () {
      toTop.classList.toggle("show", window.scrollY > 600);
    }, { passive: true });
    toTop.addEventListener("click", function () {
      window.scrollTo({ top: 0, behavior: "smooth" });
    });
  }

  /* ---------- 复制当前显示的关键词版本 ---------- */
  $$("figure.code button.copy").forEach(function (button) {
    button.addEventListener("click", function () {
      var figure = button.closest("figure.code");
      var mode = root.getAttribute("data-kw") === "en" ? "en" : "zh";
      var code = $(".kw-" + mode, figure) || $("code", figure);
      var text = code ? code.textContent : "";
      var done = function () {
        var previous = button.textContent;
        button.textContent = "已复制";
        setTimeout(function () { button.textContent = previous; }, 1200);
      };
      if (navigator.clipboard && window.isSecureContext) {
        navigator.clipboard.writeText(text).then(done, function () { fallbackCopy(text, done); });
      } else {
        fallbackCopy(text, done);
      }
    });
  });

  function fallbackCopy(text, done) {
    var area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    try { document.execCommand("copy"); done(); } catch (error) { /* 忽略 */ }
    document.body.removeChild(area);
  }

  /* ---------- 附录过滤 ---------- */
  $$("[data-filter-root]").forEach(function (bar) {
    var input = $("input.table-filter", bar);
    var count = $(".filter-count", bar);
    var rootNode = bar.parentElement;
    if (!input || !rootNode) return;

    input.addEventListener("input", function () {
      var query = input.value.trim().toLowerCase();
      var rows = $$("tbody tr", rootNode);
      var shown = 0;
      rows.forEach(function (row) {
        var match = query === "" || row.textContent.toLowerCase().indexOf(query) !== -1;
        row.hidden = !match;
        if (match) shown += 1;
      });
      $$(".table-wrap", rootNode).forEach(function (wrap) {
        wrap.hidden = $$("tbody tr", wrap).every(function (row) { return row.hidden; });
      });
      if (count) {
        count.textContent = query === "" ? "" : shown + " / " + rows.length + " 行";
      }
    });
  });
})();
