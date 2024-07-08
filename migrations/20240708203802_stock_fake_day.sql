-- Add migration script here
CREATE TABLE IF NOT EXISTS `catcoin_sim_time` (
	`id` INT PRIMARY KEY NOT NULL,
	`datetime` DATETIME NOT NULL
);
INSERT INTO `catcoin_sim_time` (`id`, `datetime`) VALUES (1, "2024-07-01T19:00:00");