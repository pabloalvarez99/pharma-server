-- pharma-server 0032: sucursales (branch) + cajas (register) — config center.
--
-- Config entities the UI can CRUD so a tenant runs multi-sucursal / multi-caja
-- from Configuración instead of CLI/seed only. Multi-tenant (reglas #4): every
-- table carries `tenant: record<tenant>` + composite index leading with tenant.
-- Append-only: never edit once applied.
--
-- A `register` (caja) MAY belong to a `branch` (sucursal) via the optional
-- `branch` link; standalone registers (no branch) are allowed for single-site
-- installs. The existing `cash_session.register_name` free-string field
-- (migration 0011) is left untouched — wiring live sessions to register records
-- is a forward step, deliberately out of scope here so the POS hot path keeps
-- zero blast radius (no FK forced on cash_session).

-- ---------------------------------------------------------------------------
-- branch (sucursal)
-- ---------------------------------------------------------------------------
DEFINE TABLE branch SCHEMAFULL;
DEFINE FIELD tenant     ON branch TYPE record<tenant>;
DEFINE FIELD name       ON branch TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD code       ON branch TYPE option<string>;
DEFINE FIELD address    ON branch TYPE option<string>;
DEFINE FIELD comuna     ON branch TYPE option<string>;
DEFINE FIELD phone      ON branch TYPE option<string>;
DEFINE FIELD active     ON branch TYPE bool DEFAULT true;
DEFINE FIELD created_at ON branch TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON branch TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX branch_tenant_name ON branch FIELDS tenant, name;

-- ---------------------------------------------------------------------------
-- register (caja)
-- ---------------------------------------------------------------------------
DEFINE TABLE register SCHEMAFULL;
DEFINE FIELD tenant     ON register TYPE record<tenant>;
DEFINE FIELD branch     ON register TYPE option<record<branch>>;
DEFINE FIELD name       ON register TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD code       ON register TYPE option<string>;
DEFINE FIELD active     ON register TYPE bool DEFAULT true;
DEFINE FIELD created_at ON register TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON register TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX register_tenant_name   ON register FIELDS tenant, name;
DEFINE INDEX register_tenant_branch ON register FIELDS tenant, branch;
