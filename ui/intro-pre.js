// Runs before anything is painted: no start-up animation for screenshots or when it is turned
// off in the settings. (A separate file, because the app's content security policy does not
// allow inline scripts.)
(function () {
  var q = new URLSearchParams(location.search), off = false;
  try { off = localStorage.getItem("stray-skip-intro") === "1"; } catch (e) {}
  if (off || q.has("nointro") || (q.has("shot") && !q.has("intro"))) document.getElementById("intro").hidden = true;
  else document.documentElement.classList.add("intro-on");
})();
