ALTER TABLE provider_authorizations ADD COLUMN superseded_display_name TEXT;
ALTER TABLE provider_authorizations ADD COLUMN superseded_endpoint TEXT;
ALTER TABLE provider_authorizations ADD COLUMN superseded_model TEXT;
ALTER TABLE provider_authorizations ADD COLUMN superseded_generation INTEGER;
ALTER TABLE provider_authorizations ADD COLUMN superseded_ready_generation INTEGER;
ALTER TABLE provider_authorizations ADD COLUMN superseded_credential_account TEXT;
