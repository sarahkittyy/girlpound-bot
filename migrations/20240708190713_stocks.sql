-- Add migration script here
CREATE TABLE IF NOT EXISTS `catcoin_company` (
	`id` INT PRIMARY KEY NOT NULL AUTO_INCREMENT,
	`name` TEXT NOT NULL,
	`tag` varchar(10) NOT NULL,
	`total_shares` INT NOT NULL,
	`logo` TEXT NOT NULL,
	`price` INT NOT NULL
);

CREATE TABLE IF NOT EXISTS `catcoin_price_history` (
	`id` INT PRIMARY KEY NOT NULL AUTO_INCREMENT,
	`company_id` INT NOT NULL,
	`price` INT NOT NULL,
	`timestamp` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	FOREIGN KEY (`company_id`) REFERENCES `catcoin_company` (`id`) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS `catcoin_user_shares` (
	`uid` varchar(32) NOT NULL,
	`company_id` INT NOT NULL,
	`shares_owned` INT NOT NULL DEFAULT 0,
	PRIMARY KEY (`uid`, `company_id`),
	FOREIGN KEY (`uid`) REFERENCES `catcoin` (`uid`) ON DELETE CASCADE ON UPDATE CASCADE,
	FOREIGN KEY (`company_id`) REFERENCES `catcoin_company` (`id`) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS `catcoin_share_listing` (
	`id` INT PRIMARY KEY NOT NULL AUTO_INCREMENT,
	`seller_id` varchar(32) NOT NULL,
	`company_id` INT NOT NULL,
	`shares_available` INT NOT NULL,
	`price_per_share` INT NOT NULL,
	`created_at` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
	`updated_at` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

	FOREIGN KEY (`seller_id`) REFERENCES `catcoin` (`uid`) ON DELETE CASCADE ON UPDATE CASCADE,
	FOREIGN KEY (`company_id`) REFERENCES `catcoin_company` (`id`) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS `catcoin_share_transactions` (
	`id` INT PRIMARY KEY NOT NULL AUTO_INCREMENT,
	`buyer_id` varchar(32) NOT NULL,
	`seller_id` varchar(32) NULL, -- can be null if buying from the company
	`company_id` INT NOT NULL,
	`shares_bought` INT NOT NULL,
	`price_per_share` INT NOT NULL,
	`created_at` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

	FOREIGN KEY (`buyer_id`) REFERENCES `catcoin` (`uid`) ON DELETE CASCADE ON UPDATE CASCADE,
	FOREIGN KEY (`seller_id`) REFERENCES `catcoin` (`uid`) ON DELETE CASCADE ON UPDATE CASCADE,
	FOREIGN KEY (`company_id`) REFERENCES `catcoin_company` (`id`) ON DELETE CASCADE ON UPDATE CASCADE
);