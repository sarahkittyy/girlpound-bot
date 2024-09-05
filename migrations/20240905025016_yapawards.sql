-- Add migration script here
CREATE TABLE IF NOT EXISTS `yapawards` (
	`uid` varchar(32) PRIMARY KEY NOT NULL,
	`count` INT NOT NULL DEFAULT 0
);