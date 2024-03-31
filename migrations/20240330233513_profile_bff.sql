-- Add migration script here
ALTER TABLE `profiles`
ADD COLUMN `favorite_user` varchar(32) NULL;