// Scaffold front end. Its only job right now is to prove the window renders and
// that the web view can reach the Rust core; the real UI arrives with task 12.
const status = document.querySelector("#boot-status");
status.textContent = "Window is up. Rust core not wired yet.";
