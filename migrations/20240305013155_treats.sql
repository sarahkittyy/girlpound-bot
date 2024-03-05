-- Add migration script here
CREATE TABLE IF NOT EXISTS `treats` (
	`uid` varchar(32) PRIMARY KEY NOT NULL,
	`treats` BIGINT NOT NULL DEFAULT 0
)