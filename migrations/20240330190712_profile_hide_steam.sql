-- Add migration script here
ALTER TABLE `profiles`
ADD COLUMN `hide_dominations` BOOLEAN NOT NULL DEFAULT false,
ADD COLUMN `hide_stats` BOOLEAN NOT NULL DEFAULT false;