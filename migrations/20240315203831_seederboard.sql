-- Add migration script here
CREATE TABLE IF NOT EXISTS `seederboard` (
	`steamid` varchar(32) PRIMARY KEY NOT NULL,
	`seconds_seeded` BIGINT NOT NULL DEFAULT 0
)