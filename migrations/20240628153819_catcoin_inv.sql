-- Add migration script here
CREATE TABLE IF NOT EXISTS `catcoin_inv` (
	`id` INT NOT NULL AUTO_INCREMENT,
	`uid` varchar(32) NOT NULL,
	`rid` INT NOT NULL,
	`number` INT NOT NULL DEFAULT 0,
	`catcoin` INT NOT NULL,
	`created_at` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	`updated_at` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	PRIMARY KEY (`id`, `uid`, `rid`),
	FOREIGN KEY (`rid`) REFERENCES `catcoin_reward` (`id`)
);