-- Add migration script here
ALTER TABLE `profiles`
ADD COLUMN `views` BIGINT UNSIGNED NOT NULL DEFAULT 0;
