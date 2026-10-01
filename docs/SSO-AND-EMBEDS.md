# Alpha single sign-on and website widgets

## Single sign-on with Alpha

One login across both products, with no shared database and no shared session.
Either side signs a short-lived assertion and the other verifies it with a
shared secret, then starts its own session.

**The assertion.** HS256 JWT: `iss` (`alpha` or `vantedge`), `aud` (the other
one), `sub` (the person's email), `tenant` (the Vantedge workspace slug), `jti`,
`iat`, `exp` (never more than 120 seconds after `iat`), and an optional `next`
path on the receiving side.

**Rules, on both sides.**
- The signature, issuer, audience and expiry must check out. The `jti` works once.
- The person must already exist, be active, and belong to the workspace (Vantedge)
  or be on the office team (Alpha). A sign-in token never creates anyone.
- Two-step sign-in still applies: Vantedge answers with the code prompt, Alpha
  refuses the token and the person signs in normally.
- Every refusal is the same 401. The reason goes to the log or audit trail.
- Landing paths (`next`) must be on the receiving site.

**Set up.**
1. Vantedge: Settings, Single sign-on, Turn on. Copy the secret (shown once).
2. Alpha: Django admin, Vantedge connections, add one with the workspace slug,
   the secret and Vantedge's web address.
3. Vantedge: on a linked vendor, enter their Alpha web address once and press
   Open in Alpha. In Alpha, Settings shows Open Vantedge.
4. Rotate the secret from the same Vantedge page; Alpha stops working until the
   new one is pasted in.

## Website widgets

A client's own website shows Vantedge content with two lines of HTML:

```html
<div data-vantedge="listings" data-tenant="your-workspace"></div>
<script async src="https://YOUR-VANTEDGE-HOST/embed.js"></script>
```

Widgets: `listings` (search and filters), `tour` (request form, optional
`data-listing`), `reviews` (Google reviews), `map` (`data-map` with a published
map id). Options: `data-accent` (hex colour) and `data-mode` (`light` or `dark`).

- Each widget is an iframe from Vantedge in the client's brand colour. It cannot
  touch the host page, and the host page cannot touch it. The loader resizes the
  frame from a height message and checks the sender's origin and frame.
- Settings, Website widgets: turn embedding on or off and list the sites allowed
  to show it. Everything in a widget is already public, so the site list stops
  other sites presenting the widgets as theirs; it is not a secret.
- The console and the public site send `frame-ancestors 'self'` and
  `X-Frame-Options: SAMEORIGIN`. Only `/embed/*` can be framed.
- Tour requests from a widget land in Tours like any other, with the same
  honeypot and consent.
