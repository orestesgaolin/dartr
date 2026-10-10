// Copy buttons for the terminal blocks. The page works without this script.
(function () {
  function fallbackCopy(text) {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    var ok = false;
    try { ok = document.execCommand("copy"); } catch (e) { ok = false; }
    document.body.removeChild(ta);
    return ok;
  }

  function done(button, label) {
    var original = button.getAttribute("data-label") || button.textContent;
    button.setAttribute("data-label", original);
    button.textContent = label;
    button.setAttribute("data-done", "");
    window.setTimeout(function () {
      button.textContent = original;
      button.removeAttribute("data-done");
    }, 1600);
  }

  document.querySelectorAll("button.copy").forEach(function (button) {
    button.hidden = false;
    button.addEventListener("click", function () {
      var target = document.getElementById(button.getAttribute("data-copy"));
      if (!target) return;
      // Copy only the commands: lines that start with a prompt.
      var lines = [];
      target.querySelectorAll("[data-cmd]").forEach(function (el) {
        lines.push(el.textContent);
      });
      var text = lines.join("\n");
      if (navigator.clipboard && window.isSecureContext) {
        navigator.clipboard.writeText(text).then(
          function () { done(button, "Copied"); },
          function () { done(button, fallbackCopy(text) ? "Copied" : "Copy failed"); }
        );
      } else {
        done(button, fallbackCopy(text) ? "Copied" : "Copy failed");
      }
    });
  });
})();
