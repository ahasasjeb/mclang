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
  if (navButton && toc) {
    navButton.addEventListener("click", function () {
      toc.classList.toggle("open");
    });
    toc.addEventListener("click", function (event) {
      if (event.target.tagName === "A") toc.classList.remove("open");
    });
  }

  /* ---------- 目录高亮当前章节 ---------- */
  var tocLinks = toc ? $$("a", toc) : [];
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
