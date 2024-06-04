-- Add migration script here
CREATE TABLE IF NOT EXISTS `catcoin_reward` (
	`id` INT PRIMARY KEY NOT NULL AUTO_INCREMENT,
	`name` TEXT NOT NULL,
	`file` TEXT NOT NULL,
	`rarity` ENUM ('Common', 'Rare', 'Fluffy', 'Peak') NOT NULL
);
INSERT INTO `catcoin_reward` (`name`, `file`, `rarity`) VALUES
("American Shorthair (Attacking Position)", "public/shorthair_attack.png", "Peak"),
("American Shorthair (Defense Position)", "public/shorthair_defense.png", "Peak"),
("American Shorthair (Seated Position)", "public/shorthair_seated.png", "Fluffy"),
("American Shorthair (Shake Position)", "public/shorthair_shake.png", "Rare"),
("Golden Poodle (Attack Position)", "public/golden_poodle_attack.png", "Rare"),
("Golden Poodle (Defense Position)", "public/golden_poodle_defense.png", "Common"),
("Golden Poodle (Tame Position)", "public/golden_poodle_tame.png", "Fluffy"),
("Birmin Kitten (Attack Position)", "public/birmin_attack.png", "Rare"),
("Birmin Kitten (Upright Position)", "public/birmin_upright.png", "Common"),
("Birmin Kitten (Seated Position)", "public/birmin_seated.png", "Common"),
("Turkish Angora (Attack Position)", "public/angora_attack.png", "Common"),
("Turkish Angora (Defense Position)", "public/angora_defense.png", "Common"),
("Turkish Angora (Seated Position)", "public/angora_seated.png", "Rare"),
("Siberian Husky (Attack Position)", "public/husky_attack.png", "Rare"),
("Siberian Husky (Seated Position)", "public/husky_seated.png", "Common"),
("Siberian Husky (Tame Position)", "public/husky_tame.png", "Rare");