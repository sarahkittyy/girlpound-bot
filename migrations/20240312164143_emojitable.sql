-- Add migration script here
CREATE TABLE IF NOT EXISTS emojirank (
	`eid` varchar(32) PRIMARY KEY NOT NULL,
	`name` TEXT NOT NULL,
	`use_count` int NOT NULL DEFAULT 0,
	`react_count` int NOT NULL DEFAULT 0,
	`is_discord` BOOLEAN NOT NULL,
	`animated` BOOLEAN NOT NULL
)