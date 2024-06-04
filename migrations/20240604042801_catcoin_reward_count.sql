-- Add migration script here
CREATE TABLE IF NOT EXISTS `catcoin_reward_count` (
	`rid` INT PRIMARY KEY NOT NULL,
	`pulls` INT NOT NULL DEFAULT 0,
	CONSTRAINT `reward_fk`
		FOREIGN KEY (`rid`) REFERENCES `catcoin_reward` (`id`)
		ON DELETE CASCADE
		ON UPDATE RESTRICT
);