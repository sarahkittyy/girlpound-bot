-- Add migration script here
CREATE TABLE IF NOT EXISTS `delete_server` (
	`uid` varchar(32) PRIMARY KEY NOT NULL,
	`vote` BOOLEAN NOT NULL
);