-- Add migration script here
CREATE TABLE IF NOT EXISTS `reminders` (
	-- message, user, channel ids
	`mid` varchar(32) PRIMARY KEY NOT NULL,
	`uid` varchar(32) NOT NULL,
	`cid` varchar(32) NOT NULL,
	`remind_at` TIMESTAMP NOT NULL
)