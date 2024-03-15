-- Add migration script here
ALTER TABLE `treats`
RENAME TO `catcoin`;

ALTER TABLE `catcoin`
RENAME COLUMN `treats` TO `catcoin`;

