-- pharma-server 0037: Free Web PR2 — storefront credentials on `api_key`.
--
-- Extends 0026's table instead of adding a parallel `web_api_key`: one key
-- table, one seam. The admin CRUD (`/api/v1/admin/web/keys`) needs a listable
-- prefix, and PR3's web orders verify per-key HMAC request signatures. Both
-- fields are optional so 0026-era CLI-minted keys stay valid.
DEFINE FIELD key_prefix  ON api_key TYPE option<string>;
DEFINE FIELD hmac_secret ON api_key TYPE option<string>;
