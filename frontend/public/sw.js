// Service worker for Web Push: shows incoming pushes as system notifications
// and focuses/opens the console when one is clicked. Payloads are JSON
// ({ title, body, url }) encrypted per RFC 8291 by the backend.

self.addEventListener("push", (event) => {
  let data = { title: "Acre Nexus", body: "", url: "/console/notifications" };
  try {
    if (event.data) data = { ...data, ...event.data.json() };
  } catch {
    if (event.data) data.body = event.data.text();
  }
  event.waitUntil(
    self.registration.showNotification(data.title, {
      body: data.body,
      data: { url: data.url },
      icon: "/favicon.ico",
      badge: "/favicon.ico",
    })
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const url = event.notification.data?.url || "/console/notifications";
  event.waitUntil(
    clients
      .matchAll({ type: "window", includeUncontrolled: true })
      .then((wins) => {
        for (const win of wins) {
          if ("focus" in win) {
            win.navigate(url);
            return win.focus();
          }
        }
        return clients.openWindow(url);
      })
  );
});
