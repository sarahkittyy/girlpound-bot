-- Add migration script here
CREATE TABLE IF NOT EXISTS dihh (
	`uid` varchar(32) PRIMARY KEY NOT NULL,
	`count` INT NOT NULL DEFAULT 0
);