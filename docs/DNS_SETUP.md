# DNS setup for `mac-storage-clear.flek.ai`

This is a one-time setup you need to do manually in Cloudflare and GitHub. Estimated time: 5 minutes setup + ~10 minutes cert provisioning.

## 1. Cloudflare DNS

Log in to Cloudflare → select the `flek.ai` zone → DNS.

Add a new record:

| Field | Value |
|---|---|
| Type | `CNAME` |
| Name | `mac-storage-clear` |
| Target | `aanya-send-help.github.io` |
| Proxy status | **DNS only** (gray cloud) — important for cert provisioning |
| TTL | Auto |

Save.

> **Why DNS only and not Proxied?** GitHub Pages provisions a Let's Encrypt cert by serving an ACME challenge on its origin. Cloudflare's proxy interferes with the challenge if enabled before the cert exists. After the cert is live and HTTPS works, you can flip back to Proxied if you want CF features — but GitHub Pages handles HTTPS fine on its own.

## 2. GitHub Pages

Go to `https://github.com/aanya-send-help/mac-storage-clear/settings/pages`.

- **Source:** GitHub Actions (we publish via the `deploy-website.yml` workflow)
- **Custom domain:** `mac-storage-clear.flek.ai` → Save

GitHub will:

1. Verify the CNAME points back to `aanya-send-help.github.io`.
2. Provision a Let's Encrypt certificate (5–15 minutes).
3. Once provisioned, the **Enforce HTTPS** checkbox becomes available — enable it.

The `website/public/CNAME` file (containing `mac-storage-clear.flek.ai`) is automatically deployed to the site root by Astro on every build, so GitHub Pages keeps the custom domain across redeploys.

## 3. Verification

After the cert is live:

```sh
curl -sI https://mac-storage-clear.flek.ai | head -1
# expect: HTTP/2 200

dig +short mac-storage-clear.flek.ai
# expect: a CNAME chain ending at github.io IPs
```

## 4. (Optional) Re-enable Cloudflare proxy

After HTTPS is confirmed working:

1. Cloudflare DNS → edit the `mac-storage-clear` record → flip Proxy status to "Proxied" (orange cloud).
2. Set SSL/TLS encryption mode (under SSL/TLS → Overview) to **Full (strict)** — GitHub Pages serves a valid cert, so strict is fine.

This enables Cloudflare's caching, DDoS protection, and bot management for the site. Skip if not needed.

## Troubleshooting

- **"Domain's DNS record could not be retrieved" in GitHub Pages settings**: CNAME has not propagated yet, or you set Cloudflare proxy on (gray cloud needed initially).
- **Cert stuck in "provisioning" >30 min**: re-confirm DNS is set to "DNS only" in Cloudflare; remove and re-add the custom domain in GitHub Pages settings.
- **HTTPS works but HTTP doesn't redirect**: enable "Enforce HTTPS" in Pages settings.
