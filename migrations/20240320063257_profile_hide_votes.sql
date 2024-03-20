-- Add migration script here
ALTER TABLE `profiles`
ADD COLUMN `hide_votes` BOOLEAN NOT NULL DEFAULT false;
