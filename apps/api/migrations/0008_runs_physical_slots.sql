-- A capacity domain is shared by this schema's three roles, not other databases
-- or test schemas. Held ownership records identify a private kernel-lock marker.
ALTER TABLE runs_slots ADD COLUMN physical_scope UUID;
UPDATE runs_slots SET physical_scope = (SELECT gen_random_uuid());
ALTER TABLE runs_slots ALTER COLUMN physical_scope SET NOT NULL;
ALTER TABLE runs_slots ADD COLUMN physical_owner JSONB;
