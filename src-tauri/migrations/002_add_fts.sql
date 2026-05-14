CREATE VIRTUAL TABLE search_index USING fts5(
    title,
    body,
    source_type UNINDEXED,
    source_id UNINDEXED,
    session_id UNINDEXED
);
