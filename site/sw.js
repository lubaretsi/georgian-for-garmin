// Exists so the page can show notifications (Android Chrome allows them only from a service
// worker). It caches nothing and handles no fetches: the page always loads from the network.

self.addEventListener("install", () => self.skipWaiting());

self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

// Tapping the notification brings the test page back.
self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil(
    (async () => {
      const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
      const page = windows.find((client) => client.url.startsWith(self.registration.scope));
      if (page) return page.focus();
      return self.clients.openWindow(self.registration.scope);
    })(),
  );
});
