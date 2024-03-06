-- Add migration script here
CREATE TABLE IF NOT EXISTS `hicat` (
	`uid` varchar(32) PRIMARY KEY NOT NULL,
	`count` int NOT NULL DEFAULT 0
)