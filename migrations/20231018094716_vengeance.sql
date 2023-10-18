-- Add migration script here
CREATE TABLE IF NOT EXISTS `domination` (
	`lt_steamid` varchar(32) NOT NULL,
	`gt_steamid` varchar(32) NOT NULL,
	`score` int DEFAULT 0 NOT NULL,
	CONSTRAINT `domination_pk` PRIMARY KEY (`lt_steamid`, `gt_steamid`)
);