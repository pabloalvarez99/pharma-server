# PR8 — Polish: SEO + demo + cierre (RutBusiness Free Web)

Engineer on **RutBusiness**. Storefront (repo `rutbusiness-storefront`) + seam
(`pharma-server` lane `feature/free-web-shopify-parity`) work end-to-end. Final pass.

## Storefront (repo `D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\rutbusiness-storefront`)

- `app/sitemap.ts` (home + product URLs from catalog fetch) · `app/robots.ts` (allow all; disallow /api).
- JSON-LD: `LocalBusiness` on home (name, address_line, hours_label, phone from store payload); `Product` + `Offer` (`priceCurrency:"CLP"`, price from `price_clp`, availability mapped schema.org) on product pages.
- `<title>`/description per page from store/product data. OG tags basic.
- Craft pass at 375px width: tap targets ≥44px, cart badge, focus states, empty states ("Tu carrito está vacío"), loading skeletons on catalog.
- Commit + push.

## Server repo (`D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web`)

- Record demo: run `scripts/web-sync/README.md` demo script top to bottom against local server; paste actual outputs into `docs/strategy/free-web-shopify-parity.md` § "Demo verificada (fecha)".
- `bitacora.md` append milestone: `- 2026-XX-XX: Free Web PR1–PR8 completo — catálogo público, keys, pedidos retiro, storefront Next. Lane feature/free-web-shopify-parity.`
- Gate (`cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace`) → commit `docs(web): PR8 demo verificada + bitácora` → push.

## Final report (print)

```
## Hito Free Web
- Paths: rutas públicas + storefront + scripts
- Demo: publish → catálogo → pedido RET-XXXX → admin transition → 404 al despublicar
- Tests: <n> passing (workspace)
- G0 offline ok (web opt-in, POS intacto) · G1 free generoso (sin 402 en web básica)
- Next: dominio propio (paid), Webpay (paid), multi-tema
```

Done → `✅ FREE WEB COMPLETO — listo para merge del lane`.
