-- Up
CREATE TABLE photo_similarity_features (
    photo_id INTEGER PRIMARY KEY,
    fingerprint TEXT NOT NULL,
    feature TEXT NOT NULL
);

-- Down
DROP TABLE photo_similarity_features;
