// Web Push subscription helpers: register the service worker, subscribe with
// the platform's VAPID key, and keep the backend's subscription registry in
// sync. All functions are safe to call in unsupported browsers (they report
// support rather than throwing).

import { api } from "./api";

export function pushSupported(): boolean {
  return (
    typeof window !== "undefined" &&
    "serviceWorker" in navigator &&
    "PushManager" in window &&
    "Notification" in window
  );
}

/** The browser hands VAPID keys over as a BufferSource. */
function urlBase64ToUint8Array(base64: string): Uint8Array {
  const padding = "=".repeat((4 - (base64.length % 4)) % 4);
  const b64 = (base64 + padding).replace(/-/g, "+").replace(/_/g, "/");
  const raw = window.atob(b64);
  return Uint8Array.from(raw, (c) => c.charCodeAt(0));
}

function keyToBase64Url(sub: PushSubscription, name: PushEncryptionKeyName) {
  const key = sub.getKey(name);
  if (!key) return "";
  const bytes = new Uint8Array(key);
  let binary = "";
  bytes.forEach((b) => (binary += String.fromCharCode(b)));
  return window
    .btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

async function registration(): Promise<ServiceWorkerRegistration> {
  const reg = await navigator.serviceWorker.register("/sw.js");
  await navigator.serviceWorker.ready;
  return reg;
}

/** The browser's current subscription, if any. */
export async function currentSubscription(): Promise<PushSubscription | null> {
  if (!pushSupported()) return null;
  const reg = await navigator.serviceWorker.getRegistration("/sw.js");
  return (await reg?.pushManager.getSubscription()) ?? null;
}

/**
 * Full enable flow: permission → service worker → subscribe with the
 * platform VAPID key → register with the backend.
 */
export async function enablePush(): Promise<void> {
  if (!pushSupported())
    throw new Error("Push is not supported in this browser");
  const permission = await Notification.requestPermission();
  if (permission !== "granted") {
    throw new Error("Notification permission was not granted");
  }
  const { key } = await api.vapidKey();
  const reg = await registration();
  const sub =
    (await reg.pushManager.getSubscription()) ??
    (await reg.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToUint8Array(key) as BufferSource,
    }));
  await api.subscribePush({
    endpoint: sub.endpoint,
    p256dh: keyToBase64Url(sub, "p256dh"),
    auth: keyToBase64Url(sub, "auth"),
  });
}

/** Disable flow: unsubscribe the browser and drop the backend registration. */
export async function disablePush(): Promise<void> {
  const sub = await currentSubscription();
  if (!sub) return;
  const endpoint = sub.endpoint;
  await sub.unsubscribe();
  try {
    await api.unsubscribePush(endpoint);
  } catch {
    // Already gone server-side — fine.
  }
}
