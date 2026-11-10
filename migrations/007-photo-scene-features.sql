-- Up
CREATE TABLE photo_scene_features (
    photo_id INTEGER PRIMARY KEY,
    fingerprint TEXT NOT NULL,
    model_version TEXT NOT NULL,
    embedding BLOB NOT NULL
);

-- Down
DROP TABLE photo_scene_features;
