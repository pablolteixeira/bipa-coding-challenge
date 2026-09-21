-- Latest imported snapshot of the mempool.space connectivity ranking.
-- Values are stored raw (sats, timestamptz); formatting happens in the API.
CREATE TABLE nodes (
    public_key    TEXT        PRIMARY KEY,
    alias         TEXT        NOT NULL,
    capacity_sats BIGINT      NOT NULL CHECK (capacity_sats >= 0),
    first_seen    TIMESTAMPTZ NOT NULL,
    -- Position in the source ranking, used to keep the API order stable.
    rank          INTEGER     NOT NULL,
    imported_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX nodes_rank_idx ON nodes (rank);
