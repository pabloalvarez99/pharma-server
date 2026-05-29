-- 0017_user_roles: granular pharmacy roles (cashier/pharmacist/admin/owner).
--
-- Schema-only change: the `roles` field already exists on `user` since
-- 0001_init (`array<string>`). This migration just normalises the values:
-- every existing user gets `[admin, owner]` so existing logins keep all
-- access they had under the binary admin/owner model. New users default
-- to `[cashier]` (least privilege) via the CLI; the DB default stays empty
-- so application logic decides explicit role assignment.

UPDATE user SET roles = ["admin", "owner"]
WHERE roles = NONE OR array::len(roles) = 0;
