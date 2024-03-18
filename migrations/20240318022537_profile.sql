-- Add migration script here
CREATE TABLE IF NOT EXISTS `profiles` (
	`uid` varchar(32) PRIMARY KEY NOT NULL, -- discord user id
	`title` TEXT NOT NULL DEFAULT "%'s profile", -- title formatter
	`url` TEXT NULL, -- profile link to wherever
	`steamid` varchar(32) NULL, -- user steamid (optional)
	`description` TEXT NULL, -- bio / profile description
	`image` TEXT NULL, -- url to an image to display
	`classes` SMALLINT(2) UNSIGNED NOT NULL DEFAULT 0 -- the classes this user plays
)