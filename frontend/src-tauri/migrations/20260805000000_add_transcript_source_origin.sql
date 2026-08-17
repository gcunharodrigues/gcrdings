ALTER TABLE transcripts ADD COLUMN source_origin TEXT;
ALTER TABLE transcripts ADD COLUMN speaker_cluster_id TEXT;
ALTER TABLE transcripts ADD COLUMN ambiguity_group_id TEXT;
ALTER TABLE transcripts ADD COLUMN alignment_decision TEXT;
