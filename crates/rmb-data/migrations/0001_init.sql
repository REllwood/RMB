-- 0001_init — base key/value store.
-- `app_meta` holds the schema/app version and small UI/app preferences (e.g. theme).
-- Feature tables (settings, customers, items, …) are added in later migrations.

CREATE TABLE app_meta (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

INSERT INTO app_meta (key, value) VALUES ('schema_version', '1');
