CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS message_schedule (
  id UUID NOT NULL,
  inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  schedule_pattern TEXT NOT NULL,
  next TIMESTAMPTZ NULL,
  transmission_count INTEGER NOT NULL,
  message TEXT NOT NULL,
  is_locked BOOLEAN NOT NULL,
  PRIMARY KEY (id, transmission_count)
);
