hljs.highlightAll();

function highlightSyntax() {
  if (typeof hljs !== 'undefined') {
    hljs.highlightAll();
  }
}

function toggleMenu() {
  const nav = document.getElementById("navcol-2");
  nav.classList.toggle("hidden");
  nav.classList.toggle("block");
}
