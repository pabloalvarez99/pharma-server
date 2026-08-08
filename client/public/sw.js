// RutBusiness web — service worker SHELL-ONLY (SP3). Cachea únicamente los
// assets estáticos del mismo origen (HTML/JS/CSS/íconos) con estrategia
// network-first; los DATOS van SIEMPRE a la red: las llamadas a la API viven en
// otro origen (api.rutbusiness.cl) y este worker jamás las intercepta ni
// cachea. Sin red y sin caché → el navegador muestra su error normal.
const CACHE = "rb-shell-v1";

self.addEventListener("install", () => {
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return; // API/data: siempre red
  if (url.pathname.startsWith("/api/") || url.pathname.startsWith("/health")) return;
  event.respondWith(
    fetch(req)
      .then((resp) => {
        if (resp.ok) {
          const copy = resp.clone();
          caches.open(CACHE).then((c) => c.put(req, copy));
        }
        return resp;
      })
      .catch(() =>
        caches.match(req).then((hit) => {
          if (hit) return hit;
          if (req.mode === "navigate") return caches.match("/index.html").then((h) => h ?? Response.error());
          return Response.error();
        }),
      ),
  );
});
