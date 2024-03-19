-- Add migration script here
CREATE TABLE IF NOT EXISTS `profile_votes` (
	`profile_uid` varchar(32) NOT NULL,
	`voter_uid` varchar(32) NOT NULL,
	`vote` TINYINT NOT NULL DEFAULT 0,
	CONSTRAINT `vote_pk` PRIMARY KEY (`profile_uid`, `voter_uid`)
);
CREATE VIEW `profile_votes_aggregate` AS
SELECT
	`profile_uid`,
	CAST(SUM(CASE WHEN vote = 1 THEN 1 ELSE 0 END) as INT) as `likes`,
	CAST(SUM(CASE WHEN vote = -1 THEN 1 ELSE 0 END) as INT) as `dislikes`
FROM `profile_votes`
GROUP BY `profile_uid`;