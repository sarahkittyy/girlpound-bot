-- Add migration script here
CREATE TABLE IF NOT EXISTS `barkers` (
	`uid` varchar(32) PRIMARY KEY NOT NULL,
	`last_nickname` varchar(128) NOT NULL,
	`updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)
